// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use move_binary_format::file_format::CompiledModule;
use move_core_types::account_address::AccountAddress;

use std::{collections::BTreeMap, path::PathBuf};
use sui_move_build::{BuildConfig, CompiledPackage};
use sui_protocol_config::{Chain, ProtocolConfig};
use sui_types::{
    base_types::ObjectID,
    digests::TransactionDigest,
    error::ExecutionErrorKind,
    execution_status::PackageUpgradeError,
    move_package::{MovePackage, TypeOrigin, UpgradeInfo},
    object::{Data, Object, OBJECT_START_VERSION},
};

macro_rules! type_origin_table {
    {} => { Vec::new() };
    {$($module:ident :: $type:ident => $pkg:expr),* $(,)?} => {{
        vec![$(TypeOrigin {
            module_name: stringify!($module).to_string(),
            datatype_name: stringify!($type).to_string(),
            package: $pkg,
        },)*]
    }}
}

macro_rules! linkage_table {
    {} => { BTreeMap::new() };
    {$($original_id:expr => ($upgraded_id:expr, $version:expr)),* $(,)?} => {{
        let mut table = BTreeMap::new();
        $(
            table.insert(
                $original_id,
                UpgradeInfo {
                    upgraded_id: $upgraded_id,
                    upgraded_version: $version,
                }
            );
        )*
        table
    }}
}

#[test]
fn test_new_initial() {
    let c_id1 = ObjectID::from_single_byte(0xc1);
    let c_pkg = MovePackage::new_initial(
        &build_test_modules("Cv1"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [],
    )
    .unwrap();

    let b_id1 = ObjectID::from_single_byte(0xb1);
    let b_pkg = MovePackage::new_initial(
        &build_test_modules("B"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [&c_pkg],
    )
    .unwrap();

    let a_pkg = MovePackage::new_initial(
        &build_test_modules("A"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [&b_pkg, &c_pkg],
    )
    .unwrap();

    assert_eq!(
        a_pkg.linkage_table(),
        &linkage_table! {
            b_id1 => (b_id1, OBJECT_START_VERSION),
            c_id1 => (c_id1, OBJECT_START_VERSION),
        }
    );

    assert_eq!(
        b_pkg.linkage_table(),
        &linkage_table! {
            c_id1 => (c_id1, OBJECT_START_VERSION),
        }
    );

    assert_eq!(c_pkg.linkage_table(), &linkage_table! {},);

    assert_eq!(
        c_pkg.type_origin_table(),
        &type_origin_table! {
            c::C => c_id1,
        }
    );

    // also test that move package sizes used for gas computations are estimated correctly (small
    // constant differences can be tolerated and are due to BCS encoding)
    let a_pkg_obj = Object::new_package_from_data(Data::Package(a_pkg), TransactionDigest::ZERO);
    let b_pkg_obj = Object::new_package_from_data(Data::Package(b_pkg), TransactionDigest::ZERO);
    let c_pkg_obj = Object::new_package_from_data(Data::Package(c_pkg), TransactionDigest::ZERO);
    let a_serialized = bcs::to_bytes(&a_pkg_obj).unwrap();
    let b_serialized = bcs::to_bytes(&b_pkg_obj).unwrap();
    let c_serialized = bcs::to_bytes(&c_pkg_obj).unwrap();
    assert_eq!(a_pkg_obj.object_size_for_gas_metering(), a_serialized.len());
    assert_eq!(b_pkg_obj.object_size_for_gas_metering(), b_serialized.len());
    assert_eq!(
        c_pkg_obj.object_size_for_gas_metering() + 2,
        c_serialized.len()
    );
}

#[test]
fn test_upgraded() {
    let c_id1 = ObjectID::from_single_byte(0xc1);
    let c_pkg = MovePackage::new_initial(
        &build_test_modules("Cv1"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [],
    )
    .unwrap();

    let c_id2 = ObjectID::from_single_byte(0xc2);
    let c_new = c_pkg
        .new_upgraded(
            c_id2,
            &build_test_modules("Cv2"),
            &ProtocolConfig::get_for_max_version_UNSAFE(),
            [],
        )
        .unwrap();

    let mut expected_version = OBJECT_START_VERSION;
    expected_version.increment();
    assert_eq!(expected_version, c_new.version());

    assert_eq!(
        c_new.type_origin_table(),
        &type_origin_table! {
            c::C => c_id1,
            c::D => c_id2,
        },
    );
}

#[test]
fn test_depending_on_upgrade() {
    let c_id1 = ObjectID::from_single_byte(0xc1);
    let c_pkg = MovePackage::new_initial(
        &build_test_modules("Cv1"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [],
    )
    .unwrap();

    let c_id2 = ObjectID::from_single_byte(0xc2);
    let c_new = c_pkg
        .new_upgraded(
            c_id2,
            &build_test_modules("Cv2"),
            &ProtocolConfig::get_for_max_version_UNSAFE(),
            [],
        )
        .unwrap();

    let b_pkg = MovePackage::new_initial(
        &build_test_modules("B"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [&c_new],
    )
    .unwrap();

    assert_eq!(
        b_pkg.linkage_table(),
        &linkage_table! {
            c_id1 => (c_id2, c_new.version()),
        },
    );
}

#[test]
fn test_upgrade_upgrades_linkage() {
    let c_id1 = ObjectID::from_single_byte(0xc1);
    let c_pkg = MovePackage::new_initial(
        &build_test_modules("Cv1"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [],
    )
    .unwrap();

    let c_id2 = ObjectID::from_single_byte(0xc2);
    let c_new = c_pkg
        .new_upgraded(
            c_id2,
            &build_test_modules("Cv2"),
            &ProtocolConfig::get_for_max_version_UNSAFE(),
            [],
        )
        .unwrap();

    let b_pkg = MovePackage::new_initial(
        &build_test_modules("B"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [&c_pkg],
    )
    .unwrap();

    let b_id2 = ObjectID::from_single_byte(0xb2);
    let b_new = b_pkg
        .new_upgraded(
            b_id2,
            &build_test_modules("B"),
            &ProtocolConfig::get_for_max_version_UNSAFE(),
            [&c_new],
        )
        .unwrap();

    assert_eq!(
        b_pkg.linkage_table(),
        &linkage_table! {
            c_id1 => (c_id1, OBJECT_START_VERSION),
        },
    );

    assert_eq!(
        b_new.linkage_table(),
        &linkage_table! {
            c_id1 => (c_id2, c_new.version()),
        },
    );
}

#[test]
fn test_upgrade_linkage_digest_to_new_dep() {
    let c_id1 = ObjectID::from_single_byte(0xc1);
    let c_pkg = MovePackage::new_initial(
        &build_test_modules("Cv1"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [],
    )
    .unwrap();

    let c_id2 = ObjectID::from_single_byte(0xc2);
    let c_new = c_pkg
        .new_upgraded(
            c_id2,
            &build_test_modules("Cv2"),
            &ProtocolConfig::get_for_max_version_UNSAFE(),
            [],
        )
        .unwrap();

    let b_pkg = MovePackage::new_initial(
        &build_test_modules("B"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [&c_pkg],
    )
    .unwrap();

    let b_id2 = ObjectID::from_single_byte(0xb2);
    let b_new = b_pkg
        .new_upgraded(
            b_id2,
            &build_test_modules("B"),
            &ProtocolConfig::get_for_max_version_UNSAFE(),
            [&c_new],
        )
        .unwrap();

    assert_eq!(
        b_new.linkage_table(),
        &linkage_table! {
            c_id1 => (c_id2, c_new.version()),
        },
    );

    // Make sure that we compute the package digest off of the update dependencies and not the old
    // dependencies in the linkage table.
    let hash_modules = true;
    assert_eq!(
        b_new.digest(hash_modules),
        MovePackage::compute_digest_for_modules_and_deps(
            &build_test_modules("B")
                .iter()
                .map(|module| {
                    let mut bytes = Vec::new();
                    module
                        .serialize_with_version(module.version, &mut bytes)
                        .unwrap();
                    bytes
                })
                .collect::<Vec<_>>(),
            [&c_id2],
            hash_modules,
        )
    )
}

#[test]
fn test_duplicate_transitive_deps() {
    use move_binary_format::file_format::basic_test_module;

    let config = ProtocolConfig::get_for_max_version_UNSAFE();
    let dep_pkg = MovePackage::new_initial(&[basic_test_module()], &config, []).unwrap();

    let pkg = MovePackage::new_initial(&[basic_test_module()], &config, [&dep_pkg, &dep_pkg]);

    assert!(pkg.is_err());

    let pkg = MovePackage::new_initial(
        &[basic_test_module()],
        &ProtocolConfig::get_for_version(88.into(), Chain::Unknown),
        [&dep_pkg, &dep_pkg],
    );

    assert!(pkg.is_ok());
}

#[test]
fn test_duplicate_transitive_deps_publish() {
    use move_binary_format::file_format::basic_test_module;

    let config = ProtocolConfig::get_for_max_version_UNSAFE();
    let dep_pkg = MovePackage::new_initial(&[basic_test_module()], &config, []).unwrap();

    let pkg = MovePackage::new_initial(&[basic_test_module()], &config, [&dep_pkg, &dep_pkg]);

    assert!(pkg.is_err());

    let pkg = MovePackage::new_initial(
        &[basic_test_module()],
        &ProtocolConfig::get_for_version(88.into(), Chain::Unknown),
        [&dep_pkg, &dep_pkg],
    );

    assert!(pkg.is_ok());
}

#[test]
fn test_transitive_dep_downgrade() {
    use move_binary_format::file_format::basic_test_module;

    let config = ProtocolConfig::get_for_max_version_UNSAFE();

    let l1 = MovePackage::new_initial(&[basic_test_module()], &config, []).unwrap();
    let l2 = l1
        .new_upgraded(
            ObjectID::from_hex_literal("0x1").unwrap(),
            &[basic_test_module()],
            &config,
            [],
        )
        .unwrap();

    let mut m = basic_test_module();
    m.address_identifiers = vec![AccountAddress::from_hex_literal("0x2").unwrap()];
    let f1 = MovePackage::new_initial(&[m.clone()], &config, [&l1]).unwrap();
    let f2 = f1
        .new_upgraded(
            ObjectID::from_hex_literal("0x3").unwrap(),
            &[m],
            &config,
            [&l2],
        )
        .unwrap();

    let pkg = MovePackage::new_initial(
        &[basic_test_module()],
        &ProtocolConfig::get_for_version(88.into(), Chain::Unknown),
        [&f1, &f2, &l1],
    )
    .unwrap();
    let linkage = pkg.linkage_table();

    assert_eq!(linkage[&l1.id()].upgraded_version, l1.version());
    assert_eq!(linkage[&l1.id()].upgraded_id, l1.id());
    assert_ne!(linkage[&l1.id()].upgraded_version, l2.version());
    assert_ne!(linkage[&l1.id()].upgraded_id, l2.id());

    assert_eq!(linkage[&f1.id()].upgraded_version, f2.version());
    assert_eq!(linkage.get(&f1.id()).unwrap().upgraded_id, f2.id());
    assert_ne!(linkage[&f1.id()].upgraded_version, f1.version());
    assert_ne!(linkage[&f1.id()].upgraded_id, f1.id());

    let pkg = MovePackage::new_initial(&[basic_test_module()], &config, [&f1, &f2, &l1]);

    assert!(pkg.is_err());
    assert_eq!(pkg.unwrap_err().kind(), &ExecutionErrorKind::InvalidLinkage);
}

#[test]
fn test_upgrade_downgrade_transitive() {
    let config = ProtocolConfig::get_for_max_version_UNSAFE();

    let c_id1 = ObjectID::from_single_byte(0xc1);
    let c_v1 = MovePackage::new_initial(&build_test_modules("Cv1"), &config, []).unwrap();

    let c_id2 = ObjectID::from_single_byte(0xc2);
    let c_v2 = c_v1
        .new_upgraded(c_id2, &build_test_modules("Cv2"), &config, [])
        .unwrap();

    let b_id1 = ObjectID::from_single_byte(0xb1);
    let b_v1 = MovePackage::new_initial(&build_test_modules("B"), &config, [&c_v1]).unwrap();

    let b_id2 = ObjectID::from_single_byte(0xb2);
    let b_v2 = b_v1
        .new_upgraded(b_id2, &build_test_modules("Bv2"), &config, [&c_v2])
        .unwrap();

    let f_id = ObjectID::from_single_byte(0xf1);
    let f_pkg =
        MovePackage::new_initial(&build_test_modules("F"), &config, [&b_v1, &c_v1]).unwrap();

    let h_pkg = MovePackage::new_initial(
        &build_test_modules("H"),
        &ProtocolConfig::get_for_version(88.into(), Chain::Unknown),
        [&f_pkg, &b_v2, &c_v1],
    )
    .unwrap();

    assert_eq!(
        h_pkg.linkage_table(),
        &linkage_table! {
            c_id1 => (c_id1, OBJECT_START_VERSION),
            b_id1 => (b_id2, b_v2.version()),
            f_id => (f_id, OBJECT_START_VERSION),
        },
    );

    assert!(
        MovePackage::new_initial(&build_test_modules("H"), &config, [&f_pkg, &b_v2, &c_v1])
            .is_err()
    );
}

#[test]
fn test_upgrade_downngrades_linkage() {
    let c_id1 = ObjectID::from_single_byte(0xc1);
    let c_pkg = MovePackage::new_initial(
        &build_test_modules("Cv1"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [],
    )
    .unwrap();

    let c_id2 = ObjectID::from_single_byte(0xc2);
    let c_new = c_pkg
        .new_upgraded(
            c_id2,
            &build_test_modules("Cv2"),
            &ProtocolConfig::get_for_max_version_UNSAFE(),
            [],
        )
        .unwrap();

    let b_pkg = MovePackage::new_initial(
        &build_test_modules("B"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [&c_new],
    )
    .unwrap();

    let b_id2 = ObjectID::from_single_byte(0xb2);
    let b_new = b_pkg
        .new_upgraded(
            b_id2,
            &build_test_modules("B"),
            &ProtocolConfig::get_for_max_version_UNSAFE(),
            [&c_pkg],
        )
        .unwrap();

    assert_eq!(
        b_pkg.linkage_table(),
        &linkage_table! {
            c_id1 => (c_id2, c_new.version()),
        },
    );

    assert_eq!(
        b_new.linkage_table(),
        &linkage_table! {
            c_id1 => (c_id1, OBJECT_START_VERSION),
        },
    );
}

#[test]
fn test_transitively_depending_on_upgrade() {
    let c_id1 = ObjectID::from_single_byte(0xc1);
    let c_pkg = MovePackage::new_initial(
        &build_test_modules("Cv1"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [],
    )
    .unwrap();

    let c_id2 = ObjectID::from_single_byte(0xc2);
    let c_new = c_pkg
        .new_upgraded(
            c_id2,
            &build_test_modules("Cv2"),
            &ProtocolConfig::get_for_max_version_UNSAFE(),
            [],
        )
        .unwrap();

    let b_id1 = ObjectID::from_single_byte(0xb1);
    let b_pkg = MovePackage::new_initial(
        &build_test_modules("B"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [&c_pkg],
    )
    .unwrap();

    let a_pkg = MovePackage::new_initial(
        &build_test_modules("A"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [&b_pkg, &c_new],
    )
    .unwrap();

    assert_eq!(
        a_pkg.linkage_table(),
        &linkage_table! {
            b_id1 => (b_id1, OBJECT_START_VERSION),
            c_id1 => (c_id2, c_new.version()),
        },
    );
}

#[test]
fn package_digest_changes_with_dep_upgrades_and_in_sync_with_move_package_digest() {
    let c_v1 = MovePackage::new_initial(
        &build_test_modules("Cv1"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [],
    )
    .unwrap();

    let c_id2 = ObjectID::from_single_byte(0xc2);
    let c_v2 = c_v1
        .new_upgraded(
            c_id2,
            &build_test_modules("Cv2"),
            &ProtocolConfig::get_for_max_version_UNSAFE(),
            [],
        )
        .unwrap();

    let b_pkg = MovePackage::new_initial(
        &build_test_modules("B"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [&c_v1],
    )
    .unwrap();
    let b_v2 = MovePackage::new_initial(
        &build_test_modules("Bv2"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [&c_v2],
    )
    .unwrap();

    let with_unpublished_deps = false;
    let local_v1 = build_test_package("B").get_package_digest(with_unpublished_deps);
    let local_v2 = build_test_package("Bv2").get_package_digest(with_unpublished_deps);

    let hash_modules = true;
    assert_ne!(b_pkg.digest(hash_modules), b_v2.digest(hash_modules));
    assert_eq!(b_pkg.digest(hash_modules), local_v1);
    assert_eq!(b_v2.digest(hash_modules), local_v2);
    assert_ne!(local_v1, local_v2);
}

#[test]
#[should_panic]
fn test_panic_on_empty_package() {
    let _ = MovePackage::new_initial(&[], &ProtocolConfig::get_for_max_version_UNSAFE(), []);
}

#[test]
fn test_fail_on_missing_dep() {
    let err = MovePackage::new_initial(
        &build_test_modules("B"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [],
    )
    .unwrap_err();

    assert_eq!(
        err.kind(),
        &ExecutionErrorKind::PublishUpgradeMissingDependency
    );
}

#[test]
fn test_fail_on_missing_transitive_dep() {
    let c_pkg = MovePackage::new_initial(
        &build_test_modules("Cv1"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [],
    )
    .unwrap();

    let b_pkg = MovePackage::new_initial(
        &build_test_modules("B"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [&c_pkg],
    )
    .unwrap();

    let err = MovePackage::new_initial(
        &build_test_modules("A"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [&b_pkg],
    )
    .unwrap_err();

    assert_eq!(
        err.kind(),
        &ExecutionErrorKind::PublishUpgradeMissingDependency
    );
}

#[test]
fn test_fail_on_transitive_dependency_downgrade() {
    let c_pkg = MovePackage::new_initial(
        &build_test_modules("Cv1"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [],
    )
    .unwrap();

    let c_id2 = ObjectID::from_single_byte(0xc2);
    let c_new = c_pkg
        .new_upgraded(
            c_id2,
            &build_test_modules("Cv2"),
            &ProtocolConfig::get_for_max_version_UNSAFE(),
            [],
        )
        .unwrap();

    let b_pkg = MovePackage::new_initial(
        &build_test_modules("B"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [&c_new],
    )
    .unwrap();

    let err = MovePackage::new_initial(
        &build_test_modules("A"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [&b_pkg, &c_pkg],
    )
    .unwrap_err();

    assert_eq!(
        err.kind(),
        &ExecutionErrorKind::PublishUpgradeDependencyDowngrade
    );
}

#[test]
fn test_fail_on_upgrade_missing_type() {
    let c_pkg = MovePackage::new_initial(
        &build_test_modules("Cv2"),
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        [],
    )
    .unwrap();

    let c_id2 = ObjectID::from_single_byte(0xc2);
    let err = c_pkg
        .new_upgraded(
            c_id2,
            &build_test_modules("Cv1"),
            &ProtocolConfig::get_for_max_version_UNSAFE(),
            [],
        )
        .unwrap_err();

    assert_eq!(
        err.kind(),
        &ExecutionErrorKind::PackageUpgradeError {
            upgrade_error: PackageUpgradeError::IncompatibleUpgrade
        }
    );

    // At versions before version 5 this was an invariant violation
    let err = c_pkg
        .new_upgraded(
            c_id2,
            &build_test_modules("Cv1"),
            &ProtocolConfig::get_for_version(4.into(), Chain::Unknown),
            [],
        )
        .unwrap_err();
    assert_eq!(err.kind(), &ExecutionErrorKind::InvariantViolation);
}

pub fn build_test_package(test_dir: &str) -> CompiledPackage {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend(["src", "unit_tests", "data", "move_package", test_dir]);
    BuildConfig::new_for_testing().build(&path).unwrap()
}

pub fn build_test_modules(test_dir: &str) -> Vec<CompiledModule> {
    build_test_package(test_dir)
        .get_modules()
        .cloned()
        .collect()
}
