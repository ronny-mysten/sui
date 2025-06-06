// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

//! Utilities for property-based testing.

use crate::file_format::{
    AddressIdentifierIndex, CompiledModule, EnumDefinition, FunctionDefinition, FunctionHandle,
    IdentifierIndex, ModuleHandle, ModuleHandleIndex, StructDefinition, TableIndex,
};
use move_core_types::{account_address::AccountAddress, identifier::Identifier};
use proptest::{
    collection::{SizeRange, btree_set, vec},
    prelude::*,
    sample::Index as PropIndex,
};

mod constants;
mod functions;
mod metadata;
mod signature;
mod types;

use constants::ConstantPoolGen;
use functions::{
    FnDefnMaterializeState, FnHandleMaterializeState, FunctionDefinitionGen, FunctionHandleGen,
};

use crate::proptest_types::{
    metadata::MetadataGen,
    signature::SignatureGen,
    types::{DatatypeHandleGen, StDefnMaterializeState, StructDefinitionGen},
};
use std::collections::{BTreeSet, HashMap};

use self::types::EnumDefinitionGen;

/// Represents how large [`CompiledModule`] tables can be.
pub type TableSize = u16;

impl CompiledModule {
    /// Convenience wrapper around [`CompiledModuleStrategyGen`][CompiledModuleStrategyGen] that
    /// generates valid modules with the given size.
    pub fn valid_strategy(size: usize) -> impl Strategy<Value = Self> {
        CompiledModuleStrategyGen::new(size as TableSize).generate()
    }
}

/// Contains configuration to generate [`CompiledModule`] instances.
///
/// If you don't care about customizing these parameters, see [`CompiledModule::valid_strategy`].
///
/// A `CompiledModule` can be looked at as a graph, with several kinds of nodes, and a nest of
/// pointers among those nodes. This graph has some properties:
///
/// 1. The graph has cycles. Generating DAGs is often simpler, but is not an option in this case.
/// 2. The actual structure of the graph is well-defined in terms of the kinds of nodes and
///    pointers that exist.
///
/// TODO: the graph also has pointers *out* of it, via address references to other modules.
/// This doesn't need to be handled when viewing modules in isolation, but some verification passes
/// will need to look at the entire set of modules. The work to make generating such modules
/// possible remains to be done.
///
/// Intermediate types
/// ------------------
///
/// The pointers are represented as indexes into vectors of other kinds of nodes. One of the
/// bigger problems is that the number of types, functions etc isn't known upfront so it is
/// impossible to know what range to pick from for the index types (`ModuleHandleIndex`,
/// `DatatypeHandleIndex`, etc). To deal with this, the code generates a bunch of intermediate
/// structures (sometimes tuples, other times more complicated structures with their own internal
/// constraints), with "holes" represented by [`Index`](proptest::sample::Index) instances. Once all
/// the lengths are known, there's a final "materialize" step at the end that "fills in" these
/// holes.
///
/// One alternative would have been to generate lengths up front, then create vectors of those
/// lengths. This would have worked fine for generation but would have made shrinking take much
/// longer, because the shrinker would be less aware of the overall structure of the problem and
/// would have ended up redoing a lot of work. The approach taken here does end up being more
/// verbose but should perform optimally.
///
/// See [`proptest` issue #130](https://github.com/AltSysrq/proptest/issues/130) for more discussion
/// about this.
#[derive(Clone, Debug)]
pub struct CompiledModuleStrategyGen {
    size: usize,
    field_count: SizeRange,
    variant_count: SizeRange,
    datatype_params: SizeRange,
    parameters_count: SizeRange,
    return_count: SizeRange,
    func_type_params: SizeRange,
    acquires_count: SizeRange,
    random_sigs_count: SizeRange,
    tokens_per_random_sig_count: SizeRange,
    code_len: SizeRange,
    jump_table_len: SizeRange,
}

impl CompiledModuleStrategyGen {
    /// Create a new configuration for randomly generating [`CompiledModule`] instances.
    pub fn new(size: TableSize) -> Self {
        Self {
            size: size as usize,
            field_count: (0..5).into(),
            variant_count: (0..5).into(),
            datatype_params: (0..3).into(),
            parameters_count: (0..4).into(),
            return_count: (0..3).into(),
            func_type_params: (0..3).into(),
            acquires_count: (0..2).into(),
            random_sigs_count: (0..5).into(),
            tokens_per_random_sig_count: (0..5).into(),
            code_len: (0..50).into(),
            jump_table_len: (0..10).into(),
        }
    }

    /// Zero out all fields, type parameters, arguments and return types of struct and functions.
    #[inline]
    pub fn zeros_all(&mut self) -> &mut Self {
        self.field_count = 0.into();
        self.variant_count = 0.into();
        self.datatype_params = 0.into();
        self.parameters_count = 0.into();
        self.return_count = 0.into();
        self.func_type_params = 0.into();
        self.acquires_count = 0.into();
        self.random_sigs_count = 0.into();
        self.tokens_per_random_sig_count = 0.into();
        self
    }

    /// Create a `proptest` strategy for `CompiledModule` instances using this configuration.
    pub fn generate(self) -> impl Strategy<Value = CompiledModule> {
        //
        // leaf pool generator
        //
        let self_idx_strat = any::<PropIndex>();
        let address_pool_strat = btree_set(any::<AccountAddress>(), 1..=self.size);
        let identifiers_strat = btree_set(any::<Identifier>(), 5..=self.size + 5);
        let constant_pool_strat = ConstantPoolGen::strategy(0..=self.size, 0..=self.size);
        let metadata_strat = MetadataGen::strategy(0..=self.size);

        // The number of PropIndex instances in each tuple represents the number of pointers out
        // from an instance of that particular kind of node.

        //
        // Module handle generator
        //
        let module_handles_strat = vec(any::<(PropIndex, PropIndex)>(), 1..=self.size);

        //
        // Struct and enum generators
        //
        let datatype_handles_strat = vec(
            DatatypeHandleGen::strategy(self.datatype_params.clone()),
            1..=self.size,
        );

        let struct_defs_strat = vec(
            StructDefinitionGen::strategy(self.field_count.clone(), self.datatype_params.clone()),
            1..=self.size / 2,
        );

        let enum_defs_strat = vec(
            EnumDefinitionGen::strategy(
                self.variant_count.clone(),
                self.field_count.clone(),
                self.datatype_params.clone(),
            ),
            1..=self.size / 2,
        );

        //
        // Random signatures generator
        //
        // These signatures may or may not be used in the bytecode. One way to use these signatures
        // is the Vec* bytecode (e.g. VecEmpty), which will grab a random index from the pool.
        //
        let random_sigs_strat = vec(
            SignatureGen::strategy(self.tokens_per_random_sig_count),
            self.random_sigs_count,
        );

        //
        // Functions generators
        //

        // FunctionHandle will add to the Signature table
        // FunctionDefinition will also add the following pool:
        // FieldHandle, StructInstantiation, FunctionInstantiation, FieldInstantiation
        let function_handles_strat = vec(
            FunctionHandleGen::strategy(
                self.parameters_count.clone(),
                self.return_count.clone(),
                self.func_type_params.clone(),
            ),
            1..=self.size,
        );
        let function_defs_strat = vec(
            FunctionDefinitionGen::strategy(
                self.return_count.clone(),
                self.parameters_count.clone(),
                self.func_type_params.clone(),
                self.acquires_count.clone(),
                self.code_len,
                self.jump_table_len,
            ),
            1..=self.size,
        );

        //
        // Friend generator
        //
        let friends_strat = vec(any::<(PropIndex, PropIndex)>(), 1..=self.size);

        // Note that prop_test only allows a tuple of length up to 10
        (
            self_idx_strat,
            (
                address_pool_strat,
                identifiers_strat,
                constant_pool_strat,
                metadata_strat,
            ),
            module_handles_strat,
            datatype_handles_strat,
            struct_defs_strat,
            enum_defs_strat,
            random_sigs_strat,
            (function_handles_strat, function_defs_strat),
            friends_strat,
        )
            .prop_map(
                |(
                    self_idx_gen,
                    (address_identifier_gens, identifier_gens, constant_pool_gen, metdata_gen),
                    module_handles_gen,
                    datatype_handle_gens,
                    struct_def_gens,
                    enum_def_gens,
                    random_sigs_gens,
                    (function_handle_gens, function_def_gens),
                    friend_decl_gens,
                )| {
                    //
                    // leaf pools
                    let address_identifiers: Vec<_> = address_identifier_gens.into_iter().collect();
                    let address_identifiers_len = address_identifiers.len();
                    let identifiers: Vec<_> = identifier_gens.into_iter().collect();
                    let identifiers_len = identifiers.len();
                    let constant_pool = constant_pool_gen.constant_pool();
                    let constant_pool_len = constant_pool.len();
                    let metadata = metdata_gen.metadata();

                    //
                    // module handles
                    let mut module_handles_set = BTreeSet::new();
                    let mut module_handles = vec![];
                    for (address, name) in module_handles_gen {
                        let mh = ModuleHandle {
                            address: AddressIdentifierIndex(
                                address.index(address_identifiers_len) as TableIndex
                            ),
                            name: IdentifierIndex(name.index(identifiers_len) as TableIndex),
                        };
                        if module_handles_set.insert((mh.address, mh.name)) {
                            module_handles.push(mh);
                        }
                    }
                    let module_handles_len = module_handles.len();

                    //
                    // self module handle index
                    let self_module_handle_idx =
                        ModuleHandleIndex(self_idx_gen.index(module_handles_len) as TableIndex);

                    //
                    // Friend Declarations
                    let friend_decl_set: BTreeSet<_> = friend_decl_gens
                        .into_iter()
                        .map(|(address_gen, name_gen)| ModuleHandle {
                            address: AddressIdentifierIndex(
                                address_gen.index(address_identifiers_len) as TableIndex,
                            ),
                            name: IdentifierIndex(name_gen.index(identifiers_len) as TableIndex),
                        })
                        .collect();
                    let friend_decls = friend_decl_set.into_iter().collect();

                    //
                    // struct handles
                    let mut datatype_handles = vec![];
                    if module_handles_len > 1 {
                        let mut datatype_handles_set = BTreeSet::new();
                        for datatype_handle_gen in datatype_handle_gens.into_iter() {
                            let sh = datatype_handle_gen.materialize(
                                self_module_handle_idx,
                                module_handles_len,
                                identifiers_len,
                            );
                            if datatype_handles_set.insert((sh.module, sh.name)) {
                                datatype_handles.push(sh);
                            }
                        }
                    }

                    //
                    // Struct definitions.
                    // Struct handles for the definitions are generated in this step
                    let mut state = StDefnMaterializeState::new(
                        self_module_handle_idx,
                        identifiers_len,
                        datatype_handles,
                    );
                    let mut struct_def_to_field_count: HashMap<usize, usize> = HashMap::new();
                    let mut struct_defs: Vec<StructDefinition> = vec![];
                    for struct_def_gen in struct_def_gens {
                        if let (Some(struct_def), offset) = struct_def_gen.materialize(&mut state) {
                            struct_defs.push(struct_def);
                            if offset > 0 {
                                struct_def_to_field_count.insert(struct_defs.len() - 1, offset);
                            }
                        }
                    }

                    let mut enum_defs: Vec<EnumDefinition> = vec![];
                    for enum_def_gen in enum_def_gens {
                        if let Some(enum_def) = enum_def_gen.materialize(&mut state) {
                            enum_defs.push(enum_def);
                        }
                    }

                    let StDefnMaterializeState {
                        datatype_handles, ..
                    } = state;

                    //
                    // Create some random signatures.
                    let mut signatures: Vec<_> = random_sigs_gens
                        .into_iter()
                        .map(|sig_gen| sig_gen.materialize(&datatype_handles))
                        .collect();

                    //
                    // Function handles.
                    let mut function_handles: Vec<FunctionHandle> = vec![];
                    if module_handles_len > 1 {
                        let mut state = FnHandleMaterializeState::new(
                            self_module_handle_idx,
                            module_handles_len,
                            identifiers_len,
                            &datatype_handles,
                            signatures,
                        );
                        for function_handle_gen in function_handle_gens {
                            if let Some(function_handle) =
                                function_handle_gen.materialize(&mut state)
                            {
                                function_handles.push(function_handle);
                            }
                        }
                        signatures = state.signatures();
                    }

                    //
                    // Function Definitions
                    // Here we need pretty much everything if we are going to emit instructions.
                    // signatures and function handles
                    let mut state = FnDefnMaterializeState::new(
                        self_module_handle_idx,
                        identifiers_len,
                        constant_pool_len,
                        &datatype_handles,
                        &struct_defs,
                        signatures,
                        function_handles,
                        struct_def_to_field_count,
                        &enum_defs,
                    );
                    let mut function_defs: Vec<FunctionDefinition> = vec![];
                    for function_def_gen in function_def_gens {
                        if let Some(function_def) = function_def_gen.materialize(&mut state) {
                            function_defs.push(function_def);
                        }
                    }
                    let (
                        signatures,
                        function_handles,
                        field_handles,
                        struct_def_instantiations,
                        function_instantiations,
                        field_instantiations,
                        enum_def_instantiations,
                        variant_handles,
                        variant_instantiation_handles,
                    ) = state.return_tables();

                    // Build a compiled module
                    CompiledModule {
                        version: crate::file_format_common::VERSION_MAX,
                        publishable: true,
                        module_handles,
                        self_module_handle_idx,
                        datatype_handles,
                        function_handles,
                        field_handles,
                        friend_decls,

                        struct_def_instantiations,
                        function_instantiations,
                        field_instantiations,

                        struct_defs,
                        function_defs,

                        signatures,

                        identifiers,
                        address_identifiers,
                        constant_pool,
                        metadata,
                        enum_defs,
                        enum_def_instantiations,
                        variant_handles,
                        variant_instantiation_handles,
                    }
                },
            )
    }
}

/// A utility function that produces a prop_index but also avoiding the given index. If the random
/// index at the first choice collides with the avoidance, then try +/- 1 from the chosen value and
/// pick the one that does not over/under-flow.
pub(crate) fn prop_index_avoid(r#gen: PropIndex, avoid: usize, pool_size: usize) -> usize {
    assert!(pool_size > 1);
    assert!(pool_size > avoid);
    let rand = r#gen.index(pool_size);
    if rand != avoid {
        return rand;
    }
    let rand_inc = rand.checked_add(1).unwrap();
    if rand_inc != pool_size {
        return rand_inc;
    }
    rand.checked_sub(1).unwrap()
}
