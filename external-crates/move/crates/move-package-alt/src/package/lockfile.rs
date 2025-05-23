// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeMap,
    ffi::OsString,
    fmt,
    fs::read_to_string,
    path::{Path, PathBuf},
};

use derive_where::derive_where;
use serde::{Deserialize, Serialize};
use serde_spanned::Spanned;
use toml_edit::{
    DocumentMut, InlineTable, Item, KeyMut, Table, Value,
    visit_mut::{VisitMut, visit_table_like_kv_mut, visit_table_mut},
};

use crate::{
    dependency::{DependencySet, ManifestDependencyInfo, PinnedDependencyInfo, pin},
    errors::{Located, LockfileError, PackageError, PackageResult, with_file},
    flavor::MoveFlavor,
};

use super::{EnvironmentName, PackageName, manifest::Manifest};

#[derive(fmt::Debug, Serialize, Deserialize)]
#[derive_where(Clone, Default)]
#[serde(bound = "")]
pub struct Lockfile<F: MoveFlavor + fmt::Debug> {
    unpublished: UnpublishedTable<F>,

    #[serde(default)]
    published: BTreeMap<EnvironmentName, Publication<F>>,
}

#[derive(fmt::Debug, Serialize, Deserialize)]
#[derive_where(Clone)]
#[serde(bound = "")]
pub struct Publication<F: MoveFlavor + fmt::Debug> {
    #[serde(flatten)]
    metadata: F::PublishedMetadata,
    dependencies: BTreeMap<PackageName, PinnedDependencyInfo<F>>,
}

#[derive(fmt::Debug, Serialize, Deserialize)]
#[derive_where(Default, Clone)]
#[serde(rename_all = "kebab-case")]
#[serde(bound = "")]
struct UnpublishedTable<F: MoveFlavor + fmt::Debug> {
    dependencies: UnpublishedDependencies<F>,

    #[serde(default)]
    dep_replacements: BTreeMap<EnvironmentName, UnpublishedDependencies<F>>,
}

#[derive(fmt::Debug, Serialize, Deserialize)]
#[derive_where(Default, Clone)]
#[serde(bound = "")]
struct UnpublishedDependencies<F: MoveFlavor + fmt::Debug> {
    #[serde(default)]
    pinned: BTreeMap<PackageName, PinnedDependencyInfo<F>>,
    #[serde(default)]
    unpinned: BTreeMap<PackageName, ManifestDependencyInfo<F>>,
}

impl<F: MoveFlavor + fmt::Debug> Lockfile<F> {
    /// Read `Move.lock` and all `Move.<env>.lock` files from the directory at `path`.
    /// Returns a new empty [Lockfile] if `path` doesn't contain a `Move.lock`.
    pub fn read_from(path: impl AsRef<Path>) -> PackageResult<Self> {
        // Parse `Move.lock`
        let lockfile_name = path.as_ref().join("Move.lock");
        if !lockfile_name.exists() {
            return Ok(Self::default());
        };

        let (result, file_id) = with_file(lockfile_name, toml_edit::de::from_str::<Self>)?;

        let Ok(mut lockfiles) = result else {
            return Err(result.unwrap_err().into());
        };

        // Add in `Move.<env>.lock` files
        let dir = std::fs::read_dir(path)?;
        for entry in dir {
            let Ok(file) = entry else { continue };

            let Some(env_name) = lockname_to_env_name(file.file_name()) else {
                continue;
            };

            let (metadata, file_id) =
                with_file(file.path(), toml_edit::de::from_str::<Publication<F>>)?;

            let Ok(metadata) = metadata else {
                return Err(metadata.unwrap_err().into());
            };

            let old_entry = lockfiles.published.insert(env_name.clone(), metadata);
            if old_entry.is_some() {
                return Err(PackageError::Generic("Move.lock and Move.{env_name}.lock both contain publication information for {env_name}".to_string()));
            }
        }

        Ok(lockfiles)
    }

    /// Serialize [self] into `Move.lock` and `Move.<env>.lock`.
    ///
    /// The [PublishedMetadata] in `self.published.<env>` are partitioned: if `env` is in [envs]
    /// then it is saved to `Move.lock` (and `Move.<env>.lock` is deleted), otherwise the metadata
    /// is stored in `Move.<env>.lock`.
    pub fn write_to(
        &self,
        path: impl AsRef<Path>,
        envs: BTreeMap<EnvironmentName, F::EnvironmentID>,
    ) -> PackageResult<()> {
        let mut output: Lockfile<F> = self.clone();
        let (pubs, locals): (BTreeMap<_, _>, BTreeMap<_, _>) = output
            .published
            .into_iter()
            .partition(|(env_name, metadata)| envs.contains_key(env_name));
        output.published = pubs;

        std::fs::write(path.as_ref().join("Move.lock"), output.render())?;

        for (chain, metadata) in locals {
            std::fs::write(
                path.as_ref().join(format!("Move.{}.lock", chain)),
                metadata.render(),
            )?;
        }

        for chain in output.published.keys() {
            let _ = std::fs::remove_file(path.as_ref().join(format!("Move.{}.lock", chain)));
        }

        Ok(())
    }

    /// Pretty-print [self] as a TOML document
    fn render(&self) -> String {
        let mut toml = toml_edit::ser::to_document(self).expect("toml serialization succeeds");

        expand_toml(&mut toml);
        // TODO: maybe this could be more concise and not duplicated in [PublishedMetadata.render]
        // by making the flattener smarter (e.g. it knows to fold anything called pinned, unpinned,
        // or dependencies, or something like that)
        flatten_toml(&mut toml["unpublished"]["dependencies"]["pinned"]);
        flatten_toml(&mut toml["unpublished"]["dependencies"]["unpinned"]);
        flatten_toml(&mut toml["unpublished"]["dependencies"]["unpinned"]);
        for (_, chain) in toml["unpublished"]["dep-replacements"]
            .as_table_like_mut()
            .unwrap()
            .iter_mut()
        {
            flatten_toml(chain.get_mut("pinned").unwrap());
            flatten_toml(chain.get_mut("unpinned").unwrap());
        }

        for (_, chain) in toml["published"].as_table_like_mut().unwrap().iter_mut() {
            flatten_toml(chain.get_mut("dependencies").unwrap());
        }

        toml.decor_mut()
            .set_prefix("# Generated by move; do not edit\n# This file should be checked in.\n\n");

        toml.to_string()
    }

    /// Return the pinned dependencies in the lockfile, including the replacements dependencies.
    // TODO: This needs to be fixed after we finalize the new design
    fn pinned_deps(&self) -> DependencySet<PinnedDependencyInfo<F>> {
        let mut dep_set = DependencySet::new();

        for (pkg_name, dep_info) in self.unpublished.dependencies.pinned.iter() {
            dep_set.insert(None, pkg_name.clone(), dep_info.clone());
        }

        for (env, deps) in &self.unpublished.dep_replacements {
            for (pkg_name, dep_info) in deps.pinned.iter() {
                dep_set.insert(Some(env.clone()), pkg_name.clone(), dep_info.clone());
            }
        }

        dep_set
    }

    /// Return the unpinned dependencies in the lockfile, including the replacements dependencies.
    // TODO: This needs to be fixed after we finalize the new design
    fn unpinned_deps(&self) -> DependencySet<ManifestDependencyInfo<F>> {
        let mut dep_set = DependencySet::new();

        for (pkg_name, dep_info) in self.unpublished.dependencies.unpinned.iter() {
            dep_set.insert(None, pkg_name.clone(), dep_info.clone());
        }

        for (env, deps) in &self.unpublished.dep_replacements {
            for (pkg_name, dep_info) in deps.unpinned.iter() {
                dep_set.insert(Some(env.clone()), pkg_name.clone(), dep_info.clone());
            }
        }

        dep_set
    }

    /// Compares the unpinned dependencies in the lockfile to [`deps`] and re-pins if they changed.
    // TODO: This needs to be fixed after we finalize the new design
    pub async fn update_lockfile(
        &mut self,
        flavor: &F,
        manifest: &Manifest<F>,
    ) -> PackageResult<()> {
        let lockfile_deps = &self.unpinned_deps();
        let lockfile_pinned_deps = &self.pinned_deps();

        let mut to_pin: DependencySet<ManifestDependencyInfo<F>> = DependencySet::new();
        let mut pinned_dep_infos: DependencySet<PinnedDependencyInfo<F>> = DependencySet::new();
        // find the dependencies that need to be pinned
        for (env, pkg, manifest_dep) in manifest.dependencies() {
            let lockfile_dep = lockfile_deps.get(&env, &pkg);
            if let Some(lockfile_dep) = lockfile_dep {
                if lockfile_dep != &manifest_dep {
                    to_pin.insert(env.clone(), pkg.clone(), manifest_dep.clone());
                } else {
                    // TODO: handle error with proper span and file path
                    let Some(pinned_dep_info) = lockfile_pinned_deps.get(&env, &pkg) else {
                        return Err(PackageError::Generic(format!(
                            "Broken lockfile. It does not contain pinned dependency for {env:?} {pkg:?}"
                        )));
                    };
                    pinned_dep_infos.insert(env.clone(), pkg.clone(), pinned_dep_info.clone());
                }
            } else {
                to_pin.insert(env.clone(), pkg.clone(), manifest_dep.clone());
            }
        }

        // pin the deps that need to be pinned
        let mut pinned_deps = pin(flavor, to_pin, manifest.environments()).await?;
        pinned_deps.extend(pinned_dep_infos);

        // convert now from `DependencySet<PinnedDependencyInfo<F>>` to unpublished pinned
        // dependencies
        // TODO: probably we want a DependencySet instead of UnpublishedTable in the Lockfile types.
        self.unpublished = UnpublishedTable::from_deps(pinned_deps, manifest.dependencies());

        Ok(())
    }

    /// Return the published metadata for all environments.
    fn published(&self) -> &BTreeMap<EnvironmentName, Publication<F>> {
        &self.published
    }

    /// Return the published metadata for a specific environment.
    pub fn published_for_env(&self, env: &EnvironmentName) -> Option<Publication<F>> {
        self.published.get(env).cloned()
    }
}

impl<F: MoveFlavor + fmt::Debug> Publication<F> {
    /// Pretty-print [self] as TOML
    fn render(&self) -> String {
        let mut toml = toml_edit::ser::to_document(self).expect("toml serialization succeeds");
        expand_toml(&mut toml);
        flatten_toml(&mut toml["dependencies"]);

        toml.decor_mut().set_prefix(
            "# Generated by move; do not edit\n# This file should not be checked in\n\n",
        );
        toml.to_string()
    }
}

// TODO: probably we want a DependencySet instead of UnpublishedTable in the Lockfile types.
impl<F: MoveFlavor> UnpublishedTable<F> {
    pub fn from_deps(
        pinned_deps: DependencySet<PinnedDependencyInfo<F>>,
        unpinned_deps: DependencySet<ManifestDependencyInfo<F>>,
    ) -> Self {
        let mut dependencies = UnpublishedDependencies::default();

        let mut dep_replacements: BTreeMap<EnvironmentName, UnpublishedDependencies<F>> =
            BTreeMap::new();

        for (env, pkg, dep) in pinned_deps {
            match env {
                // update dep replacements if there's an env
                Some(env) => {
                    dep_replacements
                        .entry(env)
                        .or_default()
                        .pinned
                        .insert(pkg, dep);
                }
                // update default pinned deps
                None => {
                    dependencies.pinned.insert(pkg, dep);
                }
            }
        }

        for (env, pkg, dep) in unpinned_deps {
            match env {
                // update dep replacements if there's an env
                Some(env) => {
                    dep_replacements
                        .entry(env)
                        .or_default()
                        .unpinned
                        .insert(pkg, dep);
                }
                // update default unpinned deps
                None => {
                    dependencies.unpinned.insert(pkg, dep);
                }
            }
        }

        Self {
            dependencies,
            dep_replacements,
        }
    }
}

/// Replace every inline table in [toml] with an implicit standard table (implicit tables are not
/// included if they have no keys directly inside them)
fn expand_toml(toml: &mut DocumentMut) {
    struct Expander;

    impl VisitMut for Expander {
        fn visit_table_mut(&mut self, table: &mut Table) {
            table.set_implicit(true);
            visit_table_mut(self, table);
        }

        fn visit_table_like_kv_mut(&mut self, mut key: KeyMut<'_>, node: &mut Item) {
            if let Item::Value(Value::InlineTable(inline_table)) = node {
                let inline_table = std::mem::replace(inline_table, InlineTable::new());
                let table = inline_table.into_table();
                key.fmt();
                *node = Item::Table(table);
            }
            visit_table_like_kv_mut(self, key, node);
        }
    }

    let mut visitor = Expander;
    visitor.visit_document_mut(toml);
}

/// Replace every table in [toml] with a non-implicit inline table.
fn flatten_toml(toml: &mut Item) {
    struct Inliner;

    impl VisitMut for Inliner {
        fn visit_table_mut(&mut self, table: &mut Table) {
            table.set_implicit(false);
            visit_table_mut(self, table);
        }

        fn visit_table_like_kv_mut(&mut self, mut key: KeyMut<'_>, node: &mut Item) {
            if let Item::Table(table) = node {
                let table = std::mem::replace(table, Table::new());
                let inline_table = table.into_inline_table();
                key.fmt();
                *node = Item::Value(Value::InlineTable(inline_table));
            }
        }
    }

    let mut visitor = Inliner;
    visitor.visit_item_mut(toml);
}

/// Given a filename of the form `Move.<env>.lock`, returns `<env>`.
fn lockname_to_env_name(filename: OsString) -> Option<String> {
    let Ok(filename) = filename.into_string() else {
        return None;
    };

    let prefix = "Move.";
    let suffix = ".lock";

    if filename.starts_with(prefix) && filename.ends_with(suffix) {
        let start_index = prefix.len();
        let end_index = filename.len() - suffix.len();

        if start_index < end_index {
            return Some(filename[start_index..end_index].to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lockname_to_env_name() {
        assert_eq!(
            lockname_to_env_name(OsString::from("Move.test.lock")),
            Some("test".to_string())
        );
        assert_eq!(
            lockname_to_env_name(OsString::from("Move.3vcga23.lock")),
            Some("3vcga23".to_string())
        );
        assert_eq!(
            lockname_to_env_name(OsString::from("Mve.test.lock.lock")),
            None
        );

        assert_eq!(lockname_to_env_name(OsString::from("Move.lock")), None);
        assert_eq!(lockname_to_env_name(OsString::from("Move.test.loc")), None);
        assert_eq!(lockname_to_env_name(OsString::from("Move.testloc")), None);
        assert_eq!(lockname_to_env_name(OsString::from("Move.test")), None);
    }
}
