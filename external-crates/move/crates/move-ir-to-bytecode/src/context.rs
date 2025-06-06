// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Result, bail, format_err};
use move_binary_format::{
    CompiledModule,
    file_format::{
        AbilitySet, AddressIdentifierIndex, CodeOffset, Constant, ConstantPoolIndex,
        DatatypeHandle, DatatypeHandleIndex, DatatypeTyParameter, EnumDefInstantiation,
        EnumDefInstantiationIndex, EnumDefinitionIndex, FieldHandle, FieldHandleIndex,
        FieldInstantiation, FieldInstantiationIndex, FunctionDefinitionIndex, FunctionHandle,
        FunctionHandleIndex, FunctionInstantiation, FunctionInstantiationIndex, FunctionSignature,
        IdentifierIndex, ModuleHandle, ModuleHandleIndex, Signature, SignatureIndex,
        SignatureToken, StructDefInstantiation, StructDefInstantiationIndex, StructDefinitionIndex,
        TableIndex, VariantHandle, VariantHandleIndex, VariantInstantiationHandle,
        VariantInstantiationHandleIndex,
    },
};
use move_bytecode_source_map::source_map::SourceMap;
use move_core_types::{
    account_address::AccountAddress,
    identifier::{IdentStr, Identifier},
};
use move_ir_types::{
    ast::{
        BlockLabel_, ConstantName, DatatypeName, Field_, FunctionName, ModuleIdent, ModuleName,
        QualifiedDatatypeIdent, VariantName,
    },
    location::Loc,
};
use std::{clone::Clone, collections::HashMap, hash::Hash};

macro_rules! get_or_add_item_macro {
    ($m:ident, $k_get:expr, $k_insert:expr) => {{
        let k_key = $k_get;
        Ok(if $m.contains_key(k_key) {
            *$m.get(k_key).unwrap()
        } else {
            let len = $m.len();
            if len >= TABLE_MAX_SIZE {
                bail!("Max table size reached!")
            }
            let index = len as TableIndex;
            $m.insert($k_insert, index);
            index
        })
    }};
}

pub const TABLE_MAX_SIZE: usize = u16::MAX as usize;
fn get_or_add_item_ref<K: Clone + Eq + Hash>(
    m: &mut HashMap<K, TableIndex>,
    k: &K,
) -> Result<TableIndex> {
    get_or_add_item_macro!(m, k, k.clone())
}

fn get_or_add_item<K: Eq + Hash>(m: &mut HashMap<K, TableIndex>, k: K) -> Result<TableIndex> {
    get_or_add_item_macro!(m, &k, k)
}

pub fn ident_str(s: &str) -> Result<&IdentStr> {
    IdentStr::new(s)
}

#[derive(Clone, Debug)]
pub struct CompiledDependencyView<'a> {
    structs: HashMap<(&'a IdentStr, &'a IdentStr), TableIndex>,
    functions: HashMap<&'a IdentStr, TableIndex>,

    module_pool: &'a [ModuleHandle],
    datatype_pool: &'a [DatatypeHandle],
    function_pool: &'a [FunctionHandle],
    identifiers: &'a [Identifier],
    address_identifiers: &'a [AccountAddress],
    signature_pool: &'a [Signature],
}

impl<'a> CompiledDependencyView<'a> {
    pub fn new(dep: &'a CompiledModule) -> Result<Self> {
        let mut structs = HashMap::new();
        let mut functions = HashMap::new();

        let self_handle = dep.self_handle_idx();

        for shandle in dep.datatype_handles() {
            let mhandle = dep.module_handle_at(shandle.module);
            let mname = dep.identifier_at(mhandle.name);
            let sname = dep.identifier_at(shandle.name);
            // get_or_add_item gets the proper struct handle index, as `dep.datatype_handles()` is
            // properly ordered
            get_or_add_item(&mut structs, (mname, sname))?;
        }

        // keep only functions defined in the current module
        // with module handle 0
        let defined_function_handles = dep
            .function_handles()
            .iter()
            .enumerate()
            .filter(|(_idx, fhandle)| fhandle.module == self_handle);
        for (idx, fhandle) in defined_function_handles {
            let fname = dep.identifier_at(fhandle.name);
            functions.insert(fname, idx as u16);
        }

        Ok(Self {
            structs,
            functions,
            module_pool: dep.module_handles(),
            datatype_pool: dep.datatype_handles(),
            function_pool: dep.function_handles(),
            identifiers: dep.identifiers(),
            address_identifiers: dep.address_identifiers(),
            signature_pool: dep.signatures(),
        })
    }

    fn source_struct_info(&self, idx: DatatypeHandleIndex) -> Option<(ModuleIdent, DatatypeName)> {
        let handle = self.datatype_pool.get(idx.0 as usize)?;
        let module_handle = self.module_pool.get(handle.module.0 as usize)?;
        let address = *self
            .address_identifiers
            .get(module_handle.address.0 as usize)?;
        let module = ModuleName(
            self.identifiers
                .get(module_handle.name.0 as usize)?
                .as_str()
                .into(),
        );
        assert!(module != ModuleName::module_self());
        let ident = ModuleIdent {
            address,
            name: module,
        };
        let name = DatatypeName(
            self.identifiers
                .get(handle.name.0 as usize)?
                .as_str()
                .into(),
        );
        Some((ident, name))
    }

    fn datatype_handle(
        &self,
        module: &ModuleName,
        name: &DatatypeName,
    ) -> Option<&'a DatatypeHandle> {
        self.structs
            .get(&(
                ident_str(module.0.as_str()).ok()?,
                ident_str(name.0.as_str()).ok()?,
            ))
            .and_then(|idx| self.datatype_pool.get(*idx as usize))
    }

    fn function_signature(&self, name: &FunctionName) -> Option<FunctionSignature> {
        self.functions
            .get(ident_str(name.0.as_str()).ok()?)
            .and_then(|idx| {
                let fh = self.function_pool.get(*idx as usize)?;
                Some(FunctionSignature {
                    parameters: self.signature_pool[fh.parameters.0 as usize].0.clone(),
                    return_: self.signature_pool[fh.return_.0 as usize].0.clone(),
                    type_parameters: fh.type_parameters.clone(),
                })
            })
    }
}

#[ouroboros::self_referencing]
pub(crate) struct StoredCompiledDependency {
    module: Box<CompiledModule>,
    #[borrows(module)]
    #[covariant]
    view: CompiledDependencyView<'this>,
}

impl StoredCompiledDependency {
    pub fn create(module: CompiledModule) -> Result<Self> {
        Self::try_new(Box::new(module), move |module| {
            CompiledDependencyView::new(module)
        })
    }
}

pub(crate) enum CompiledDependency<'a> {
    /// Simple `CompiledDependencyView` where the borrowed `CompiledModule` is held elsewhere,
    /// Commonly, it is borrowed from outside of the compilers API
    Borrowed(CompiledDependencyView<'a>),
    /// `Stored` holds the `CompiledModule` as well as the `CompiledDependencyView` into the module
    /// uses `rental` for a self referential struct
    /// This is used to solve an issue of creating a `CompiledModule` and immediately needing to
    /// borrow it for the `CompiledDependencyView`. The `StoredCompiledDependency` gets around this
    /// by storing the module in it's first field, and then it's second field borrows the value in
    /// the first field via the `rental` crate
    Stored(StoredCompiledDependency),
}

impl<'a> CompiledDependency<'a> {
    pub fn borrowed(module: &'a CompiledModule) -> Result<Self> {
        Ok(Self::Borrowed(CompiledDependencyView::new(module)?))
    }

    pub fn stored(module: CompiledModule) -> Result<Self> {
        Ok(Self::Stored(StoredCompiledDependency::create(module)?))
    }
}

pub(crate) type CompiledDependencies<'a> = HashMap<ModuleIdent, CompiledDependency<'a>>;

/// Represents all of the pools to be used in the file format, both by CompiledModule
/// and CompiledScript.
pub struct MaterializedPools {
    /// Module handle pool
    pub module_handles: Vec<ModuleHandle>,
    /// Struct handle pool
    pub datatype_handles: Vec<DatatypeHandle>,
    /// Function handle pool
    pub function_handles: Vec<FunctionHandle>,
    /// Field handle pool
    pub field_handles: Vec<FieldHandle>,
    /// Struct instantiation pool
    pub struct_def_instantiations: Vec<StructDefInstantiation>,
    /// Enum instantiation pool
    pub enum_def_instantiations: Vec<EnumDefInstantiation>,
    /// Function instantiation pool
    pub function_instantiations: Vec<FunctionInstantiation>,
    /// Field instantiation pool
    pub field_instantiations: Vec<FieldInstantiation>,
    /// Locals signatures pool
    pub signatures: Vec<Signature>,
    /// Identifier pool
    pub identifiers: Vec<Identifier>,
    /// Address identifier pool
    pub address_identifiers: Vec<AccountAddress>,
    /// Constant pool
    pub constant_pool: Vec<Constant>,
    pub variant_handles: Vec<VariantHandle>,
    pub variant_instantiation_handles: Vec<VariantInstantiationHandle>,
}

/// Compilation context for a single compilation unit (module or script).
/// Contains all of the pools as they are built up.
/// Specific definitions to CompiledModule or CompiledScript are not stored.
/// However, some fields, like struct_defs and fields, are not used in CompiledScript.
pub(crate) struct Context<'a> {
    dependencies: CompiledDependencies<'a>,
    publishable: bool,

    // helpers
    aliases: HashMap<ModuleIdent, ModuleName>,
    modules: HashMap<ModuleName, (ModuleIdent, ModuleHandle)>,
    structs: HashMap<QualifiedDatatypeIdent, DatatypeHandle>,
    struct_defs: HashMap<DatatypeName, TableIndex>,
    enum_defs: HashMap<DatatypeName, TableIndex>,
    named_constants: HashMap<ConstantName, TableIndex>,
    labels: HashMap<BlockLabel_, u16>,

    // queryable pools
    // TODO: lookup for Fields is not that seemless after binary format changes
    // We need multiple lookups or a better representation for fields
    fields: HashMap<(DatatypeHandleIndex, Field_), (StructDefinitionIndex, SignatureToken, usize)>,
    function_handles: HashMap<(ModuleName, FunctionName), (FunctionHandle, FunctionHandleIndex)>,
    function_signatures: HashMap<(ModuleName, FunctionName), FunctionSignature>,
    variants: HashMap<(DatatypeHandleIndex, VariantName), (EnumDefinitionIndex, usize, usize)>,

    // Simple pools
    module_handles: HashMap<ModuleHandle, TableIndex>,
    datatype_handles: HashMap<DatatypeHandle, TableIndex>,
    signatures: HashMap<Signature, TableIndex>,
    identifiers: HashMap<Identifier, TableIndex>,
    address_identifiers: HashMap<AccountAddress, TableIndex>,
    constant_pool: HashMap<Constant, TableIndex>,
    field_handles: HashMap<FieldHandle, TableIndex>,
    struct_instantiations: HashMap<StructDefInstantiation, TableIndex>,
    enum_instantiations: HashMap<EnumDefInstantiation, TableIndex>,
    function_instantiations: HashMap<FunctionInstantiation, TableIndex>,
    field_instantiations: HashMap<FieldInstantiation, TableIndex>,
    variant_handles: HashMap<VariantHandle, TableIndex>,
    variant_instantiation_handles: HashMap<VariantInstantiationHandle, TableIndex>,

    // The current function index that we are on
    current_function_index: FunctionDefinitionIndex,

    // Source location mapping for this module
    pub source_map: SourceMap,
}

impl<'a> Context<'a> {
    /// Given the dependencies and the current module, creates an empty context.
    /// The current module is a dummy `Self` for CompiledScript.
    /// It initializes an "import" of `Self` as the alias for the current_module.
    pub fn new(
        decl_location: Loc,
        publishable: bool,
        dependencies: CompiledDependencies<'a>,
        current_module: ModuleIdent,
    ) -> Result<Self> {
        let context = Self {
            dependencies,
            publishable,
            aliases: HashMap::new(),
            modules: HashMap::new(),
            structs: HashMap::new(),
            struct_defs: HashMap::new(),
            enum_defs: HashMap::new(),
            named_constants: HashMap::new(),
            labels: HashMap::new(),
            fields: HashMap::new(),
            variants: HashMap::new(),
            function_handles: HashMap::new(),
            function_signatures: HashMap::new(),
            module_handles: HashMap::new(),
            datatype_handles: HashMap::new(),
            field_handles: HashMap::new(),
            struct_instantiations: HashMap::new(),
            enum_instantiations: HashMap::new(),
            function_instantiations: HashMap::new(),
            field_instantiations: HashMap::new(),
            signatures: HashMap::new(),
            identifiers: HashMap::new(),
            address_identifiers: HashMap::new(),
            constant_pool: HashMap::new(),
            current_function_index: FunctionDefinitionIndex::new(0),
            source_map: SourceMap::new(decl_location, current_module),
            variant_handles: HashMap::new(),
            variant_instantiation_handles: HashMap::new(),
        };

        Ok(context)
    }

    pub fn publishable(&self) -> bool {
        self.publishable
    }

    pub fn take_dependencies(&mut self) -> CompiledDependencies<'a> {
        std::mem::take(&mut self.dependencies)
    }

    pub fn restore_dependencies(&mut self, dependencies: CompiledDependencies<'a>) {
        assert!(self.dependencies.is_empty());
        self.dependencies = dependencies;
    }

    pub fn add_compiled_dependency(&mut self, compiled_dep: &'a CompiledModule) -> Result<()> {
        let ident = ModuleIdent {
            address: *compiled_dep.address(),
            name: ModuleName(compiled_dep.name().as_str().into()),
        };
        match self.dependencies.get(&ident) {
            None => self
                .dependencies
                .insert(ident, CompiledDependency::borrowed(compiled_dep)?),
            Some(_previous) => bail!("Duplicate dependency module for {}", ident),
        };
        Ok(())
    }

    fn materialize_pool<T: Clone>(
        size: usize,
        items: impl IntoIterator<Item = (T, TableIndex)>,
    ) -> Vec<T> {
        let mut options = vec![None; size];
        for (item, idx) in items {
            assert!(options[idx as usize].is_none());
            options[idx as usize] = Some(item);
        }
        options.into_iter().map(|opt| opt.unwrap()).collect()
    }

    fn materialize_map<T: Clone>(m: HashMap<T, TableIndex>) -> Vec<T> {
        Self::materialize_pool(m.len(), m)
    }

    /// Finish compilation, and materialize the pools for file format.
    pub fn materialize_pools(self) -> (MaterializedPools, CompiledDependencies<'a>, SourceMap) {
        let num_functions = self.function_handles.len();
        assert!(num_functions == self.function_signatures.len());
        let function_handles = Self::materialize_pool(
            num_functions,
            self.function_handles
                .into_iter()
                .map(|(_, (t, idx))| (t, idx.0)),
        );
        let materialized_pools = MaterializedPools {
            function_handles,
            module_handles: Self::materialize_map(self.module_handles),
            datatype_handles: Self::materialize_map(self.datatype_handles),
            field_handles: Self::materialize_map(self.field_handles),
            signatures: Self::materialize_map(self.signatures),
            identifiers: Self::materialize_map(self.identifiers),
            address_identifiers: Self::materialize_map(self.address_identifiers),
            constant_pool: Self::materialize_map(self.constant_pool),
            function_instantiations: Self::materialize_map(self.function_instantiations),
            struct_def_instantiations: Self::materialize_map(self.struct_instantiations),
            enum_def_instantiations: Self::materialize_map(self.enum_instantiations),
            field_instantiations: Self::materialize_map(self.field_instantiations),
            variant_handles: Self::materialize_map(self.variant_handles),
            variant_instantiation_handles: Self::materialize_map(
                self.variant_instantiation_handles,
            ),
        };
        (materialized_pools, self.dependencies, self.source_map)
    }

    pub fn build_index_remapping(
        &mut self,
        label_to_index: HashMap<BlockLabel_, u16>,
    ) -> HashMap<u16, u16> {
        let labels = std::mem::take(&mut self.labels);
        label_to_index
            .into_iter()
            .map(|(lbl, actual_idx)| (labels[&lbl], actual_idx))
            .collect()
    }

    //**********************************************************************************************
    // Pools
    //**********************************************************************************************

    /// Get the alias for the identifier, fails if it is not bound.
    fn module_alias(&self, ident: &ModuleIdent) -> Result<&ModuleName> {
        self.aliases
            .get(ident)
            .ok_or_else(|| format_err!("Missing import for module {}", ident))
    }

    /// Get the handle for the alias, fails if it is not bound.
    fn module_handle(&self, module_name: &ModuleName) -> Result<&ModuleHandle> {
        match self.modules.get(module_name) {
            None => bail!("Unbound module alias {}", module_name),
            Some((_, mh)) => Ok(mh),
        }
    }

    /// Get the identifier for the alias, fails if it is not bound.
    pub fn module_ident(&self, module_name: &ModuleName) -> Result<&ModuleIdent> {
        match self.modules.get(module_name) {
            None => bail!("Unbound module alias {}", module_name),
            Some((id, _)) => Ok(id),
        }
    }

    /// Get the module handle index for the alias, fails if it is not bound.
    pub fn module_handle_index(&self, module_name: &ModuleName) -> Result<ModuleHandleIndex> {
        Ok(ModuleHandleIndex(
            *self
                .module_handles
                .get(self.module_handle(module_name)?)
                .unwrap(),
        ))
    }

    /// Get the field handle index for the alias, adds it if missing.
    pub fn field_handle_index(
        &mut self,
        owner: StructDefinitionIndex,
        field: u16,
    ) -> Result<FieldHandleIndex> {
        let field_handle = FieldHandle { owner, field };
        Ok(FieldHandleIndex(get_or_add_item(
            &mut self.field_handles,
            field_handle,
        )?))
    }

    pub fn variant_handle_index(
        &mut self,
        enum_def: EnumDefinitionIndex,
        variant: u16,
    ) -> Result<VariantHandleIndex> {
        let variant_handle = VariantHandle { enum_def, variant };
        Ok(VariantHandleIndex(get_or_add_item(
            &mut self.variant_handles,
            variant_handle,
        )?))
    }

    pub fn variant_instantiation_handle_index(
        &mut self,
        enum_def: EnumDefInstantiationIndex,
        variant: u16,
    ) -> Result<VariantInstantiationHandleIndex> {
        let variant_handle = VariantInstantiationHandle { enum_def, variant };
        Ok(VariantInstantiationHandleIndex(get_or_add_item(
            &mut self.variant_instantiation_handles,
            variant_handle,
        )?))
    }

    /// Get the struct instantiation index for the alias, adds it if missing.
    pub fn struct_instantiation_index(
        &mut self,
        def: StructDefinitionIndex,
        type_parameters: SignatureIndex,
    ) -> Result<StructDefInstantiationIndex> {
        let struct_inst = StructDefInstantiation {
            def,
            type_parameters,
        };
        Ok(StructDefInstantiationIndex(get_or_add_item(
            &mut self.struct_instantiations,
            struct_inst,
        )?))
    }

    /// Get the enum instantiation index for the alias, adds it if missing.
    pub fn enum_instantiation_index(
        &mut self,
        def: EnumDefinitionIndex,
        type_parameters: SignatureIndex,
    ) -> Result<EnumDefInstantiationIndex> {
        let enum_inst = EnumDefInstantiation {
            def,
            type_parameters,
        };
        Ok(EnumDefInstantiationIndex(get_or_add_item(
            &mut self.enum_instantiations,
            enum_inst,
        )?))
    }

    /// Get the function instantiation index for the alias, adds it if missing.
    pub fn function_instantiation_index(
        &mut self,
        handle: FunctionHandleIndex,
        type_parameters: SignatureIndex,
    ) -> Result<FunctionInstantiationIndex> {
        let func_inst = FunctionInstantiation {
            handle,
            type_parameters,
        };
        Ok(FunctionInstantiationIndex(get_or_add_item(
            &mut self.function_instantiations,
            func_inst,
        )?))
    }

    /// Get the field instantiation index for the alias, adds it if missing.
    pub fn field_instantiation_index(
        &mut self,
        handle: FieldHandleIndex,
        type_parameters: SignatureIndex,
    ) -> Result<FieldInstantiationIndex> {
        let field_inst = FieldInstantiation {
            handle,
            type_parameters,
        };
        Ok(FieldInstantiationIndex(get_or_add_item(
            &mut self.field_instantiations,
            field_inst,
        )?))
    }

    /// Get the fake offset for the label. Labels will be fixed to real offsets after compilation
    pub fn label_index(&mut self, label: BlockLabel_) -> Result<CodeOffset> {
        get_or_add_item(&mut self.labels, label)
    }

    /// Get the identifier pool index, adds it if missing.
    pub fn identifier_index(&mut self, s: impl AsRef<str>) -> Result<IdentifierIndex> {
        let ident = ident_str(s.as_ref())?;
        let m = &mut self.identifiers;
        let idx: Result<TableIndex> = get_or_add_item_macro!(m, ident, ident.to_owned());
        Ok(IdentifierIndex(idx?))
    }

    /// Get the address pool index, adds it if missing.
    pub fn address_index(&mut self, addr: AccountAddress) -> Result<AddressIdentifierIndex> {
        Ok(AddressIdentifierIndex(get_or_add_item(
            &mut self.address_identifiers,
            addr,
        )?))
    }

    /// Get the byte array pool index, adds it if missing.
    #[allow(clippy::ptr_arg)]
    pub fn constant_index(&mut self, constant: Constant) -> Result<ConstantPoolIndex> {
        Ok(ConstantPoolIndex(get_or_add_item(
            &mut self.constant_pool,
            constant,
        )?))
    }

    pub fn named_constant_index(&mut self, constant: &ConstantName) -> Result<ConstantPoolIndex> {
        match self.named_constants.get(constant) {
            None => bail!("Missing constant definition for {}", constant),
            Some(idx) => Ok(ConstantPoolIndex(*idx)),
        }
    }

    /// Get the field index, fails if it is not bound.
    pub fn field(
        &self,
        s: DatatypeHandleIndex,
        f: Field_,
    ) -> Result<(StructDefinitionIndex, SignatureToken, usize)> {
        match self.fields.get(&(s, f.clone())) {
            None => bail!("Unbound field {}", f),
            Some((sd_idx, token, decl_order)) => Ok((*sd_idx, token.clone(), *decl_order)),
        }
    }

    pub fn variant(
        &self,
        s: DatatypeHandleIndex,
        f: VariantName,
    ) -> Result<(EnumDefinitionIndex, usize)> {
        match self.variants.get(&(s, f.clone())) {
            None => bail!("Unbound variant {}", f),
            Some((edef, _, tag)) => Ok((*edef, *tag)),
        }
    }

    /// Get the struct definition index, fails if it is not bound.
    pub fn struct_definition_index(&self, s: &DatatypeName) -> Result<StructDefinitionIndex> {
        match self.struct_defs.get(s) {
            None => bail!("Missing struct definition for {}", s),
            Some(idx) => Ok(StructDefinitionIndex(*idx)),
        }
    }

    pub fn enum_definition_index(&self, s: &DatatypeName) -> Result<EnumDefinitionIndex> {
        match self.enum_defs.get(s) {
            None => bail!("Missing enum definition for {}", s),
            Some(idx) => Ok(EnumDefinitionIndex(*idx)),
        }
    }

    /// Get the signature pool index, adds it if missing.
    pub fn signature_index(&mut self, sig: Signature) -> Result<SignatureIndex> {
        Ok(SignatureIndex(get_or_add_item(&mut self.signatures, sig)?))
    }

    pub fn set_function_index(&mut self, index: TableIndex) {
        self.current_function_index = FunctionDefinitionIndex(index);
    }

    pub fn current_function_definition_index(&self) -> FunctionDefinitionIndex {
        self.current_function_index
    }

    pub fn current_struct_definition_index(&self) -> StructDefinitionIndex {
        let idx = self.struct_defs.len();
        StructDefinitionIndex(idx as TableIndex)
    }

    pub fn current_enum_definition_index(&self) -> EnumDefinitionIndex {
        let idx = self.enum_defs.len();
        EnumDefinitionIndex(idx as TableIndex)
    }

    //**********************************************************************************************
    // Declarations
    //**********************************************************************************************

    /// Add a friend. This creates a module handle for the friended module.
    pub fn declare_friend(&mut self, id: ModuleIdent) -> Result<ModuleHandle> {
        let address = self.address_index(id.address)?;
        let name = self.identifier_index(id.name.0)?;
        Ok(ModuleHandle { address, name })
    }

    /// Add an import. This creates a module handle index for the imported module.
    pub fn declare_import(
        &mut self,
        id: ModuleIdent,
        alias: ModuleName,
    ) -> Result<ModuleHandleIndex> {
        // We don't care about duplicate aliases, if they exist
        self.aliases.insert(id, alias);
        let address = self.address_index(id.address)?;
        let name = self.identifier_index(id.name.0)?;
        self.modules
            .insert(alias, (id, ModuleHandle { address, name }));
        Ok(ModuleHandleIndex(get_or_add_item_ref(
            &mut self.module_handles,
            &self.modules.get(&alias).unwrap().1,
        )?))
    }

    /// Given an identifier and basic "signature" information, creates a struct handle
    /// and adds it to the pool.
    pub fn declare_datatype_handle_index(
        &mut self,
        sname: QualifiedDatatypeIdent,
        abilities: AbilitySet,
        type_parameters: Vec<DatatypeTyParameter>,
    ) -> Result<DatatypeHandleIndex> {
        self.declare_datatype_handle_index_with_abilities(sname, abilities, type_parameters)
    }

    fn declare_datatype_handle_index_with_abilities(
        &mut self,
        sname: QualifiedDatatypeIdent,
        abilities: AbilitySet,
        type_parameters: Vec<DatatypeTyParameter>,
    ) -> Result<DatatypeHandleIndex> {
        let module = self.module_handle_index(&sname.module)?;
        let name = self.identifier_index(sname.name.0)?;
        self.structs.insert(
            sname.clone(),
            DatatypeHandle {
                module,
                name,
                abilities,
                type_parameters,
            },
        );
        Ok(DatatypeHandleIndex(get_or_add_item_ref(
            &mut self.datatype_handles,
            self.structs.get(&sname).unwrap(),
        )?))
    }

    /// Given an identifier, declare the struct definition index.
    pub fn declare_struct_definition_index(
        &mut self,
        s: DatatypeName,
    ) -> Result<StructDefinitionIndex> {
        let idx = self.struct_defs.len();
        if idx > TABLE_MAX_SIZE {
            bail!("too many struct definitions {}", s)
        }
        // TODO: Add the decl of the struct definition name here
        // need to handle duplicates
        Ok(StructDefinitionIndex(
            *self.struct_defs.entry(s).or_insert(idx as TableIndex),
        ))
    }

    /// Given an identifier, declare the enum definition index.
    pub fn declare_enum_definition_index(
        &mut self,
        s: DatatypeName,
    ) -> Result<EnumDefinitionIndex> {
        let idx = self.enum_defs.len();
        if idx > TABLE_MAX_SIZE {
            bail!("too many struct definitions {}", s)
        }
        // TODO: Add the decl of the struct definition name here
        // need to handle duplicates
        Ok(EnumDefinitionIndex(
            *self.enum_defs.entry(s).or_insert(idx as TableIndex),
        ))
    }

    /// Given an identifier and a signature, creates a function handle and adds it to the pool.
    /// Finds the index for the signature, or adds it to the pool if an identical one has not yet
    /// been used.
    pub fn declare_function(
        &mut self,
        mname: ModuleName,
        fname: FunctionName,
        signature: FunctionSignature,
    ) -> Result<()> {
        let m_f = (mname, fname.clone());
        let module = self.module_handle_index(&mname)?;
        let name = self.identifier_index(fname.0)?;

        self.function_signatures
            .insert(m_f.clone(), signature.clone());

        let FunctionSignature {
            return_,
            parameters,
            type_parameters,
        } = signature;

        let params_idx = get_or_add_item(&mut self.signatures, Signature(parameters))?;
        let return_idx = get_or_add_item(&mut self.signatures, Signature(return_))?;

        let handle = FunctionHandle {
            module,
            name,
            parameters: SignatureIndex(params_idx as TableIndex),
            return_: SignatureIndex(return_idx as TableIndex),
            type_parameters,
        };
        // handle duplicate declarations
        // erroring on duplicates needs to be done by the bytecode verifier
        let hidx = match self.function_handles.get(&m_f) {
            None => self.function_handles.len(),
            Some((_, idx)) => idx.0 as usize,
        };
        if hidx > TABLE_MAX_SIZE {
            bail!("too many functions: {}.{}", mname, fname)
        }
        let handle_index = FunctionHandleIndex(hidx as TableIndex);
        self.function_handles.insert(m_f, (handle, handle_index));

        Ok(())
    }

    /// Given a named constant, adds it to the pool
    pub fn declare_constant(&mut self, name: ConstantName, constant: Constant) -> Result<()> {
        let idx = self.constant_index(constant)?;
        self.named_constants.insert(name, idx.0);
        Ok(())
    }

    /// Given a struct handle and a field, adds it to the pool.
    pub fn declare_field(
        &mut self,
        s: DatatypeHandleIndex,
        sd_idx: StructDefinitionIndex,
        f: Field_,
        token: SignatureToken,
        decl_order: usize,
    ) {
        // need to handle duplicates
        self.fields
            .entry((s, f))
            .or_insert((sd_idx, token, decl_order));
    }

    pub fn declare_variant(
        &mut self,
        s: DatatypeHandleIndex,
        ed_idx: EnumDefinitionIndex,
        f: VariantName,
        field_count: usize,
        tag: usize,
    ) {
        // need to handle duplicates
        self.variants
            .entry((s, f))
            .or_insert((ed_idx, field_count, tag));
    }

    //**********************************************************************************************
    // Dependency Resolution
    //**********************************************************************************************

    fn dependency(&self, m: &ModuleIdent) -> Result<&CompiledDependencyView> {
        let dep = self
            .dependencies
            .get(m)
            .ok_or_else(|| format_err!("Dependency not provided for {}", m))?;
        Ok(match dep {
            CompiledDependency::Borrowed(v) => v,
            CompiledDependency::Stored(stored) => stored.borrow_view(),
        })
    }

    fn dep_datatype_handle(
        &mut self,
        s: &QualifiedDatatypeIdent,
    ) -> Result<(AbilitySet, Vec<DatatypeTyParameter>)> {
        if s.module == ModuleName::module_self() {
            bail!("Unbound struct {}", s)
        }
        let mident = *self.module_ident(&s.module)?;
        let dep = self.dependency(&mident)?;
        match dep.datatype_handle(&mident.name, &s.name) {
            None => bail!("Unbound struct {}", s),
            Some(shandle) => Ok((shandle.abilities, shandle.type_parameters.clone())),
        }
    }

    /// Given an identifier, find the struct handle index.
    /// Creates the handle and adds it to the pool if it it is the *first* time it looks
    /// up the struct in a dependency.
    pub fn datatype_handle_index(
        &mut self,
        s: QualifiedDatatypeIdent,
    ) -> Result<DatatypeHandleIndex> {
        match self.structs.get(&s) {
            Some(sh) => Ok(DatatypeHandleIndex(*self.datatype_handles.get(sh).unwrap())),
            None => {
                let (abilities, type_parameters) = self.dep_datatype_handle(&s)?;
                self.declare_datatype_handle_index_with_abilities(s, abilities, type_parameters)
            }
        }
    }

    fn reindex_signature_token(
        &mut self,
        dep: &ModuleIdent,
        orig: SignatureToken,
    ) -> Result<SignatureToken> {
        Ok(match orig {
            x @ SignatureToken::Bool
            | x @ SignatureToken::U8
            | x @ SignatureToken::U16
            | x @ SignatureToken::U32
            | x @ SignatureToken::U64
            | x @ SignatureToken::U128
            | x @ SignatureToken::U256
            | x @ SignatureToken::Address
            | x @ SignatureToken::Signer
            | x @ SignatureToken::TypeParameter(_) => x,
            SignatureToken::Vector(inner) => {
                let correct_inner = self.reindex_signature_token(dep, *inner)?;
                SignatureToken::Vector(Box::new(correct_inner))
            }
            SignatureToken::Reference(inner) => {
                let correct_inner = self.reindex_signature_token(dep, *inner)?;
                SignatureToken::Reference(Box::new(correct_inner))
            }
            SignatureToken::MutableReference(inner) => {
                let correct_inner = self.reindex_signature_token(dep, *inner)?;
                SignatureToken::MutableReference(Box::new(correct_inner))
            }
            SignatureToken::Datatype(orig_sh_idx) => {
                let dep_info = self.dependency(dep)?;
                let (mident, sname) = dep_info
                    .source_struct_info(orig_sh_idx)
                    .ok_or_else(|| format_err!("Malformed dependency"))?;
                let module_name = *self.module_alias(&mident)?;
                let sident = QualifiedDatatypeIdent {
                    module: module_name,
                    name: sname,
                };
                let correct_sh_idx = self.datatype_handle_index(sident)?;
                SignatureToken::Datatype(correct_sh_idx)
            }
            SignatureToken::DatatypeInstantiation(inst) => {
                let (orig_sh_idx, inners) = *inst;
                let dep_info = self.dependency(dep)?;
                let (mident, sname) = dep_info
                    .source_struct_info(orig_sh_idx)
                    .ok_or_else(|| format_err!("Malformed dependency"))?;
                let module_name = *self.module_alias(&mident)?;
                let sident = QualifiedDatatypeIdent {
                    module: module_name,
                    name: sname,
                };
                let correct_sh_idx = self.datatype_handle_index(sident)?;
                let correct_inners = inners
                    .into_iter()
                    .map(|t| self.reindex_signature_token(dep, t))
                    .collect::<Result<_>>()?;
                SignatureToken::DatatypeInstantiation(Box::new((correct_sh_idx, correct_inners)))
            }
        })
    }

    fn reindex_function_signature(
        &mut self,
        dep: &ModuleIdent,
        orig: FunctionSignature,
    ) -> Result<FunctionSignature> {
        let return_ = orig
            .return_
            .into_iter()
            .map(|t| self.reindex_signature_token(dep, t))
            .collect::<Result<_>>()?;
        let parameters = orig
            .parameters
            .into_iter()
            .map(|t| self.reindex_signature_token(dep, t))
            .collect::<Result<_>>()?;
        let type_parameters = orig.type_parameters;
        Ok(FunctionSignature {
            return_,
            parameters,
            type_parameters,
        })
    }

    fn dep_function_signature(
        &mut self,
        m: &ModuleName,
        f: &FunctionName,
    ) -> Result<FunctionSignature> {
        if m == &ModuleName::module_self() {
            bail!("Unbound function {}.{}", m, f)
        }
        let mident = *self.module_ident(m)?;
        let dep = self.dependency(&mident)?;
        match dep.function_signature(f) {
            None => bail!("Unbound function {}.{}", mident, f),
            Some(sig) => self.reindex_function_signature(&mident, sig),
        }
    }

    fn ensure_function_declared(&mut self, m: ModuleName, f: FunctionName) -> Result<()> {
        let m_f = (m, f.clone());
        if !self.function_handles.contains_key(&m_f) {
            assert!(!self.function_signatures.contains_key(&m_f));
            let sig = self.dep_function_signature(&m, &f)?;
            self.declare_function(m, f, sig)?;
        }

        assert!(self.function_handles.contains_key(&m_f));
        assert!(self.function_signatures.contains_key(&m_f));
        Ok(())
    }

    /// Given an identifier, find the function handle and its index.
    /// Creates the handle+signature and adds it to the pool if it it is the *first* time it looks
    /// up the function in a dependency.
    pub fn function_handle(
        &mut self,
        m: ModuleName,
        f: FunctionName,
    ) -> Result<&(FunctionHandle, FunctionHandleIndex)> {
        self.ensure_function_declared(m, f.clone())?;
        Ok(self.function_handles.get(&(m, f)).unwrap())
    }

    pub fn decl_location(&self) -> Loc {
        self.source_map.definition_location
    }
}
