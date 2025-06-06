// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    compatibility::{Compatibility, InclusionCheck, Mark, compare_ord_iters},
    file_format::*,
    normalized::{self, RcIdentifier, RcPool, Type},
};
use move_core_types::{account_address::AccountAddress, ident_str, identifier::Identifier};
use proptest::prelude::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    convert::TryFrom,
    rc::Rc,
};

type NormalizedModule = normalized::Module<RcIdentifier>;

// A way to permute pools, and index into them still.
pub struct Permutation {
    permutation: Vec<u16>,
    inverse: Vec<u16>,
}

impl Permutation {
    pub fn new(permutation: Vec<u16>) -> Self {
        let inverse: Vec<_> = (0..permutation.len() as u16)
            .map(|i| permutation.iter().position(|p| *p == i).unwrap() as u16)
            .collect();
        Self {
            permutation,
            inverse,
        }
    }

    pub fn pool<T: Clone>(&self, pool: Vec<T>) -> Vec<T> {
        (0..pool.len() as u16)
            .map(|i| pool[*self.permutation.get(i as usize).unwrap_or(&i) as usize].clone())
            .collect()
    }

    pub fn permute(&self, i: u16) -> u16 {
        // If we don't have it in the permutation default to the identity
        *self.inverse.get(i as usize).unwrap_or(&i)
    }
}

fn mk_module(pool: &mut RcPool, vis: u8) -> NormalizedModule {
    mk_module_entry(pool, vis, false)
}

fn max_version(mut module: NormalizedModule) -> NormalizedModule {
    module.file_format_version = crate::file_format_common::VERSION_MAX;
    module
}

fn mk_module_entry(pool: &mut RcPool, vis: u8, is_entry: bool) -> NormalizedModule {
    let (visibility, is_entry) = if vis == Visibility::DEPRECATED_SCRIPT {
        (Visibility::Public, true)
    } else {
        (Visibility::try_from(vis).unwrap(), is_entry)
    };
    let m = CompiledModule {
        version: crate::file_format_common::VERSION_4,
        publishable: true,
        module_handles: vec![
            // only self module
            ModuleHandle {
                address: AddressIdentifierIndex(0),
                name: IdentifierIndex(0),
            },
        ],
        self_module_handle_idx: ModuleHandleIndex(0),
        identifiers: vec![
            Identifier::new("M").unwrap(),  // Module name
            Identifier::new("fn").unwrap(), // Function name
        ],
        address_identifiers: vec![
            AccountAddress::ZERO, // Module address
        ],
        function_handles: vec![
            // fun fn()
            FunctionHandle {
                module: ModuleHandleIndex(0),
                name: IdentifierIndex(1),
                parameters: SignatureIndex(0),
                return_: SignatureIndex(0),
                type_parameters: vec![],
            },
        ],
        function_defs: vec![
            // public(script) fun fn() { return; }
            FunctionDefinition {
                function: FunctionHandleIndex(0),
                visibility,
                is_entry,
                acquires_global_resources: vec![],
                code: Some(CodeUnit {
                    locals: SignatureIndex(0),
                    code: vec![
                        Bytecode::LdConst(ConstantPoolIndex(0)),
                        Bytecode::LdConst(ConstantPoolIndex(1)),
                        Bytecode::LdConst(ConstantPoolIndex(2)),
                        Bytecode::Ret,
                    ],
                    jump_tables: vec![],
                }),
            },
        ],
        signatures: vec![
            Signature(vec![]),                    // void
            Signature(vec![SignatureToken::U64]), // u64
        ],
        struct_defs: vec![],
        datatype_handles: vec![],
        constant_pool: vec![
            Constant {
                type_: SignatureToken::U8,
                data: vec![0],
            },
            Constant {
                type_: SignatureToken::U8,
                data: vec![1],
            },
            Constant {
                type_: SignatureToken::Bool,
                data: vec![1],
            },
        ],
        metadata: vec![],
        field_handles: vec![],
        friend_decls: vec![],
        struct_def_instantiations: vec![],
        function_instantiations: vec![],
        field_instantiations: vec![],
        enum_defs: vec![],
        enum_def_instantiations: vec![],
        variant_handles: vec![],
        variant_instantiation_handles: vec![],
    };
    NormalizedModule::new(pool, &m, /* include code */ true)
}

fn mk_module_plus_code(pool: &mut RcPool, vis: u8, code: Vec<Bytecode>) -> NormalizedModule {
    mk_module_plus_code_perm(pool, vis, code, Permutation::new(vec![]))
}

fn mk_module_plus_code_perm(
    pool: &mut RcPool,
    vis: u8,
    code: Vec<Bytecode>,
    p: Permutation,
) -> NormalizedModule {
    let (visibility, is_entry) = if vis == Visibility::DEPRECATED_SCRIPT {
        (Visibility::Public, true)
    } else {
        (Visibility::try_from(vis).unwrap(), false)
    };
    let m = CompiledModule {
        version: crate::file_format_common::VERSION_4,
        publishable: true,
        module_handles: vec![
            // only self module
            ModuleHandle {
                address: AddressIdentifierIndex(0),
                name: IdentifierIndex(p.permute(0)),
            },
        ],
        self_module_handle_idx: ModuleHandleIndex(0),
        identifiers: p.pool(vec![
            Identifier::new("M").unwrap(),   // Module name
            Identifier::new("fn").unwrap(),  // Function name
            Identifier::new("fn1").unwrap(), // Function name
        ]),
        address_identifiers: vec![
            AccountAddress::ZERO, // Module address
        ],
        function_handles: p.pool(vec![
            // fun fn()
            FunctionHandle {
                module: ModuleHandleIndex(0),
                name: IdentifierIndex(p.permute(1)),
                parameters: SignatureIndex(p.permute(0)),
                return_: SignatureIndex(p.permute(0)),
                type_parameters: vec![],
            },
            FunctionHandle {
                module: ModuleHandleIndex(0),
                name: IdentifierIndex(p.permute(2)),
                parameters: SignatureIndex(p.permute(1)),
                return_: SignatureIndex(p.permute(0)),
                type_parameters: vec![],
            },
        ]),
        function_defs: p.pool(vec![
            // public(script) fun fn1(u64) { return; }
            FunctionDefinition {
                function: FunctionHandleIndex(p.permute(1)),
                visibility,
                is_entry,
                acquires_global_resources: vec![],
                code: Some(CodeUnit {
                    locals: SignatureIndex(p.permute(0)),
                    code,
                    jump_tables: vec![],
                }),
            },
            // public(script) fun fn() { return; }
            FunctionDefinition {
                function: FunctionHandleIndex(p.permute(0)),
                visibility,
                is_entry,
                acquires_global_resources: vec![],
                code: Some(CodeUnit {
                    locals: SignatureIndex(p.permute(0)),
                    code: vec![
                        Bytecode::LdConst(ConstantPoolIndex(p.permute(0))),
                        Bytecode::LdConst(ConstantPoolIndex(p.permute(1))),
                        Bytecode::LdConst(ConstantPoolIndex(p.permute(2))),
                        Bytecode::Ret,
                    ],
                    jump_tables: vec![],
                }),
            },
        ]),
        signatures: p.pool(vec![
            Signature(vec![]),                    // void
            Signature(vec![SignatureToken::U64]), // u64
        ]),
        struct_defs: vec![],
        datatype_handles: vec![],
        constant_pool: p.pool(vec![
            Constant {
                type_: SignatureToken::U8,
                data: vec![0],
            },
            Constant {
                type_: SignatureToken::U8,
                data: vec![1],
            },
            Constant {
                type_: SignatureToken::Bool,
                data: vec![1],
            },
        ]),
        metadata: vec![],
        field_handles: vec![],
        friend_decls: vec![],
        struct_def_instantiations: vec![],
        function_instantiations: vec![],
        field_instantiations: vec![],
        enum_defs: vec![],
        enum_def_instantiations: vec![],
        variant_handles: vec![],
        variant_instantiation_handles: vec![],
    };
    NormalizedModule::new(pool, &m, /* include code */ true)
}

fn mk_module_plus(pool: &mut RcPool, vis: u8) -> NormalizedModule {
    mk_module_plus_code(pool, vis, vec![Bytecode::Ret])
}

fn mk_module_plus_perm(pool: &mut RcPool, vis: u8, permutation: Permutation) -> NormalizedModule {
    mk_module_plus_code_perm(pool, vis, vec![Bytecode::Ret], permutation)
}

fn make_complex_module_perm(pool: &mut RcPool, p: Permutation) -> NormalizedModule {
    let m = CompiledModule {
        version: crate::file_format_common::VERSION_MAX,
        publishable: true,
        module_handles: vec![
            // only self module
            ModuleHandle {
                address: AddressIdentifierIndex(0),
                name: IdentifierIndex(p.permute(0)),
            },
        ],
        self_module_handle_idx: ModuleHandleIndex(0),
        identifiers: p.pool(vec![
            Identifier::new("M").unwrap(),       // Module name
            Identifier::new("S").unwrap(),       // Struct name
            Identifier::new("GS").unwrap(),      // Generic struct name
            Identifier::new("R").unwrap(),       // Resource name
            Identifier::new("GR").unwrap(),      // Generic resource name
            Identifier::new("f").unwrap(),       // Field name
            Identifier::new("fn").unwrap(),      // Function name
            Identifier::new("g_fn").unwrap(),    // Generic function name
            Identifier::new("test_fn").unwrap(), // Test function name
        ]),
        address_identifiers: vec![
            AccountAddress::ZERO, // Module address
        ],
        datatype_handles: p.pool(vec![
            DatatypeHandle {
                module: ModuleHandleIndex(0),
                name: IdentifierIndex(p.permute(1)),
                abilities: AbilitySet::PRIMITIVES,
                type_parameters: vec![],
            },
            DatatypeHandle {
                module: ModuleHandleIndex(0),
                name: IdentifierIndex(p.permute(2)),
                abilities: AbilitySet::PRIMITIVES,
                type_parameters: vec![DatatypeTyParameter {
                    constraints: AbilitySet::PRIMITIVES,
                    is_phantom: false,
                }],
            },
            DatatypeHandle {
                module: ModuleHandleIndex(0),
                name: IdentifierIndex(p.permute(3)),
                abilities: AbilitySet::EMPTY | Ability::Key,
                type_parameters: vec![],
            },
            DatatypeHandle {
                module: ModuleHandleIndex(0),
                name: IdentifierIndex(p.permute(4)),
                abilities: AbilitySet::EMPTY | Ability::Key,
                type_parameters: vec![DatatypeTyParameter {
                    constraints: AbilitySet::PRIMITIVES,
                    is_phantom: false,
                }],
            },
            DatatypeHandle {
                module: ModuleHandleIndex(0),
                name: IdentifierIndex(p.permute(5)),
                abilities: AbilitySet::PRIMITIVES,
                type_parameters: vec![],
            },
            DatatypeHandle {
                module: ModuleHandleIndex(0),
                name: IdentifierIndex(p.permute(6)),
                abilities: AbilitySet::PRIMITIVES,
                type_parameters: vec![DatatypeTyParameter {
                    constraints: AbilitySet::PRIMITIVES,
                    is_phantom: false,
                }],
            },
            DatatypeHandle {
                module: ModuleHandleIndex(0),
                name: IdentifierIndex(p.permute(7)),
                abilities: AbilitySet::PRIMITIVES,
                type_parameters: vec![],
            },
            DatatypeHandle {
                module: ModuleHandleIndex(0),
                name: IdentifierIndex(p.permute(8)),
                abilities: AbilitySet::PRIMITIVES,
                type_parameters: vec![],
            },
        ]),
        struct_defs: p.pool(vec![
            // struct S { f: u64 }
            StructDefinition {
                struct_handle: DatatypeHandleIndex(p.permute(0)),
                field_information: StructFieldInformation::Declared(vec![FieldDefinition {
                    name: IdentifierIndex(p.permute(5)),
                    signature: TypeSignature(SignatureToken::U64),
                }]),
            },
            // struct GS<T> { f: T }
            StructDefinition {
                struct_handle: DatatypeHandleIndex(p.permute(1)),
                field_information: StructFieldInformation::Declared(vec![FieldDefinition {
                    name: IdentifierIndex(p.permute(5)),
                    signature: TypeSignature(SignatureToken::TypeParameter(0)),
                }]),
            },
            // struct R has key { f: u64 }
            StructDefinition {
                struct_handle: DatatypeHandleIndex(p.permute(2)),
                field_information: StructFieldInformation::Declared(vec![FieldDefinition {
                    name: IdentifierIndex(p.permute(5)),
                    signature: TypeSignature(SignatureToken::U64),
                }]),
            },
            // struct GR<T> has key { f: T }
            StructDefinition {
                struct_handle: DatatypeHandleIndex(p.permute(3)),
                field_information: StructFieldInformation::Declared(vec![FieldDefinition {
                    name: IdentifierIndex(p.permute(5)),
                    signature: TypeSignature(SignatureToken::TypeParameter(0)),
                }]),
            },
        ]),
        function_handles: p.pool(vec![
            // fun fn()
            FunctionHandle {
                module: ModuleHandleIndex(0),
                name: IdentifierIndex(p.permute(6)),
                parameters: SignatureIndex(p.permute(0)),
                return_: SignatureIndex(p.permute(0)),
                type_parameters: vec![],
            },
            // fun g_fn<T: key>(): u64
            FunctionHandle {
                module: ModuleHandleIndex(0),
                name: IdentifierIndex(p.permute(7)),
                parameters: SignatureIndex(p.permute(0)),
                return_: SignatureIndex(p.permute(2)),
                type_parameters: vec![AbilitySet::EMPTY | Ability::Key],
            },
            // fun test_fn(Sender)
            FunctionHandle {
                module: ModuleHandleIndex(0),
                name: IdentifierIndex(p.permute(8)),
                parameters: SignatureIndex(p.permute(1)),
                return_: SignatureIndex(p.permute(0)),
                type_parameters: vec![],
            },
        ]),
        function_defs: p.pool(vec![
            // public fun fn() { return; }
            FunctionDefinition {
                function: FunctionHandleIndex(p.permute(0)),
                visibility: Visibility::Public,
                is_entry: false,
                acquires_global_resources: vec![],
                code: Some(CodeUnit {
                    locals: SignatureIndex(p.permute(0)),
                    code: vec![Bytecode::Ret],
                    jump_tables: vec![],
                }),
            },
            // fun g_fn<T>() { return; }
            FunctionDefinition {
                function: FunctionHandleIndex(p.permute(1)),
                visibility: Visibility::Private,
                is_entry: false,
                acquires_global_resources: vec![],
                code: Some(CodeUnit {
                    locals: SignatureIndex(p.permute(0)),
                    code: vec![Bytecode::Ret],
                    jump_tables: vec![],
                }),
            },
            FunctionDefinition {
                function: FunctionHandleIndex(p.permute(2)),
                visibility: Visibility::Private,
                is_entry: false,
                acquires_global_resources: vec![],
                code: Some(CodeUnit {
                    locals: SignatureIndex(p.permute(0)),
                    code: vec![],
                    jump_tables: vec![],
                }),
            },
        ]),
        signatures: p.pool(vec![
            Signature(vec![]),                        // void
            Signature(vec![SignatureToken::Address]), // address
            Signature(vec![SignatureToken::U64]),     // u64
        ]),
        constant_pool: p.pool(vec![
            Constant {
                type_: SignatureToken::U8,
                data: vec![0],
            },
            Constant {
                type_: SignatureToken::U8,
                data: vec![1],
            },
            Constant {
                type_: SignatureToken::Bool,
                data: vec![1],
            },
            // an address
            Constant {
                type_: SignatureToken::Address,
                data: AccountAddress::random().to_vec(),
            },
        ]),
        metadata: vec![],
        field_handles: vec![],
        friend_decls: vec![],
        struct_def_instantiations: vec![],
        function_instantiations: vec![],
        field_instantiations: vec![],
        enum_defs: p.pool(vec![
            EnumDefinition {
                enum_handle: DatatypeHandleIndex(p.permute(4)),
                variants: vec![VariantDefinition {
                    variant_name: IdentifierIndex(p.permute(0)),
                    fields: vec![FieldDefinition {
                        name: IdentifierIndex(p.permute(4)),
                        signature: TypeSignature(SignatureToken::U64),
                    }],
                }],
            },
            EnumDefinition {
                enum_handle: DatatypeHandleIndex(p.permute(5)),
                variants: vec![VariantDefinition {
                    variant_name: IdentifierIndex(p.permute(0)),
                    fields: vec![
                        FieldDefinition {
                            name: IdentifierIndex(p.permute(5)),
                            signature: TypeSignature(SignatureToken::U64),
                        },
                        FieldDefinition {
                            name: IdentifierIndex(p.permute(3)),
                            signature: TypeSignature(SignatureToken::TypeParameter(0)),
                        },
                    ],
                }],
            },
            EnumDefinition {
                enum_handle: DatatypeHandleIndex(p.permute(6)),
                variants: vec![VariantDefinition {
                    variant_name: IdentifierIndex(p.permute(0)),
                    fields: vec![FieldDefinition {
                        name: IdentifierIndex(p.permute(4)),
                        signature: TypeSignature(SignatureToken::U64),
                    }],
                }],
            },
            EnumDefinition {
                enum_handle: DatatypeHandleIndex(p.permute(7)),
                variants: vec![VariantDefinition {
                    variant_name: IdentifierIndex(p.permute(0)),
                    fields: vec![FieldDefinition {
                        name: IdentifierIndex(p.permute(4)),
                        signature: TypeSignature(SignatureToken::U64),
                    }],
                }],
            },
        ]),
        enum_def_instantiations: vec![],
        variant_handles: p.pool(vec![
            VariantHandle {
                enum_def: EnumDefinitionIndex(p.permute(0)),
                variant: 0,
            },
            VariantHandle {
                enum_def: EnumDefinitionIndex(p.permute(1)),
                variant: 0,
            },
            VariantHandle {
                enum_def: EnumDefinitionIndex(p.permute(2)),
                variant: 0,
            },
            VariantHandle {
                enum_def: EnumDefinitionIndex(p.permute(3)),
                variant: 0,
            },
        ]),
        variant_instantiation_handles: vec![],
    };
    NormalizedModule::new(pool, &m, /* include code */ true)
}

fn mk_module_with_defs(
    pool: &mut RcPool,
    struct_defs: Vec<(Identifier, StructDefinition)>,
    enum_defs: Vec<(Identifier, EnumDefinition)>,
    function_defs: Vec<(Identifier, FunctionDefinition)>,
    friend_defs: Vec<(Identifier, AccountAddress)>,
) -> NormalizedModule {
    let mut identifiers = vec![
        Identifier::new("M").unwrap(), // Module name
    ];

    let mut datatype_handles = vec![];

    let struct_defs: Vec<StructDefinition> = struct_defs
        .into_iter()
        .map(|(name, def)| {
            identifiers.push(name);
            let i = identifiers.len() - 1;
            datatype_handles.push(DatatypeHandle {
                module: ModuleHandleIndex(0),
                name: IdentifierIndex(i as TableIndex),
                abilities: AbilitySet::EMPTY,
                type_parameters: vec![],
            });
            def
        })
        .collect();

    let enum_defs: Vec<EnumDefinition> = enum_defs
        .into_iter()
        .map(|(name, def)| {
            identifiers.push(name);
            let i = identifiers.len() - 1;
            datatype_handles.push(DatatypeHandle {
                module: ModuleHandleIndex(0),
                name: IdentifierIndex(i as TableIndex),
                abilities: AbilitySet::EMPTY,
                type_parameters: vec![],
            });
            def
        })
        .collect();

    let function_defs: Vec<FunctionDefinition> = function_defs
        .into_iter()
        .map(|(name, def)| {
            identifiers.push(name);
            let i = identifiers.len() - 1;
            datatype_handles.push(DatatypeHandle {
                module: ModuleHandleIndex(0),
                name: IdentifierIndex(i as TableIndex),
                abilities: AbilitySet::EMPTY,
                type_parameters: vec![],
            });
            def
        })
        .collect();

    let mut address_identifiers = vec![
        AccountAddress::ZERO, // self
    ];
    let mut friend_decls = vec![];

    for name in &friend_defs {
        identifiers.push(name.0.clone());
        address_identifiers.push(name.1);
        friend_decls.push(ModuleHandle {
            address: AddressIdentifierIndex(address_identifiers.len() as TableIndex - 1),
            name: IdentifierIndex(identifiers.len() as TableIndex - 1),
        });
    }

    let m = CompiledModule {
        version: crate::file_format_common::VERSION_MAX,
        publishable: true,
        module_handles: vec![
            // only self module
            ModuleHandle {
                address: AddressIdentifierIndex(0),
                name: IdentifierIndex(0),
            },
        ],
        self_module_handle_idx: ModuleHandleIndex(0),
        identifiers,
        address_identifiers,
        function_handles: vec![
            // fun fn()
            FunctionHandle {
                module: ModuleHandleIndex(0),
                name: IdentifierIndex(1),
                parameters: SignatureIndex(0),
                return_: SignatureIndex(0),
                type_parameters: vec![],
            },
        ],
        function_defs,
        signatures: vec![
            Signature(vec![]),                    // void
            Signature(vec![SignatureToken::U64]), // u64
        ],
        struct_defs,
        datatype_handles,
        constant_pool: vec![
            Constant {
                type_: SignatureToken::U8,
                data: vec![0],
            },
            Constant {
                type_: SignatureToken::U8,
                data: vec![1],
            },
            Constant {
                type_: SignatureToken::Bool,
                data: vec![1],
            },
        ],
        metadata: vec![],
        field_handles: vec![],
        friend_decls,
        struct_def_instantiations: vec![],
        function_instantiations: vec![],
        field_instantiations: vec![],
        enum_defs,
        enum_def_instantiations: vec![],
        variant_handles: vec![],
        variant_instantiation_handles: vec![],
    };
    NormalizedModule::new(pool, &m, /* include code */ true)
}

#[test]
fn deprecated_unchanged_script_visibility() {
    let pool = &mut RcPool::new();
    let script_module = mk_module(pool, Visibility::DEPRECATED_SCRIPT);
    assert!(
        Compatibility::full_check()
            .check(&script_module, &script_module)
            .is_ok(),
    );
}

#[test]
fn deprecated_remove_script_visibility() {
    let pool = &mut RcPool::new();
    let script_module = mk_module(pool, Visibility::DEPRECATED_SCRIPT);
    // script -> private, not allowed
    let private_module = mk_module(pool, Visibility::Private as u8);
    assert!(
        Compatibility::full_check()
            .check(&script_module, &private_module)
            .is_err()
    );
    // script -> public, not allowed
    let public_module = mk_module(pool, Visibility::Public as u8);
    assert!(
        Compatibility::full_check()
            .check(&script_module, &public_module)
            .is_err()
    );
    // script -> friend, not allowed
    let friend_module = mk_module(pool, Visibility::Friend as u8);
    assert!(
        Compatibility::full_check()
            .check(&script_module, &friend_module)
            .is_err()
    );
}

#[test]
fn deprecated_add_script_visibility() {
    let pool = &mut RcPool::new();
    let script_module = mk_module(pool, Visibility::DEPRECATED_SCRIPT);
    // private -> script, allowed
    let private_module = mk_module(pool, Visibility::Private as u8);
    assert!(
        Compatibility::full_check()
            .check(&private_module, &script_module)
            .is_ok()
    );
    // public -> script, not allowed
    let public_module = mk_module(pool, Visibility::Public as u8);
    assert!(
        Compatibility::full_check()
            .check(&public_module, &script_module)
            .is_err()
    );
    // friend -> script, not allowed
    let friend_module = mk_module(pool, Visibility::Friend as u8);
    assert!(
        Compatibility::full_check()
            .check(&friend_module, &script_module)
            .is_err()
    );
}

#[test]
fn private_entry_to_public_entry_allowed() {
    let pool = &mut RcPool::new();
    let private_module = max_version(mk_module_entry(pool, Visibility::Private as u8, true));
    let public_module = max_version(mk_module_entry(pool, Visibility::Public as u8, true));
    assert!(
        Compatibility::full_check()
            .check(&private_module, &public_module)
            .is_ok()
    );

    assert!(
        Compatibility::full_check()
            .check(&public_module, &private_module)
            .is_err()
    );
}

#[test]
fn public_loses_entry() {
    let pool = &mut RcPool::new();
    let public_entry = max_version(mk_module_entry(pool, Visibility::Public as u8, true));
    let public = max_version(mk_module_entry(pool, Visibility::Public as u8, false));
    assert!(
        Compatibility::full_check()
            .check(&public, &public_entry)
            .is_ok()
    );

    assert!(
        Compatibility::full_check()
            .check(&public_entry, &public)
            .is_err()
    );
}

#[test]
fn private_entry_signature_change_allowed() {
    let pool = &mut RcPool::new();
    // Create a private entry function
    let module = max_version(mk_module_entry(pool, Visibility::Private as u8, true));
    let mut updated_module = module.clone();
    // Update the signature of the entry fun to now take a u64 argument.
    let function_ref = updated_module.functions.get_mut(ident_str!("fn")).unwrap();
    let new_function = {
        let mut f: normalized::Function<_> = (**function_ref).clone();
        f.parameters = Rc::new(vec![Rc::new(Type::U64)]);
        Rc::new(f)
    };
    *function_ref = new_function;

    // allow updating signatures of private entry functions
    assert!(
        Compatibility {
            check_datatype_layout: true,
            check_private_entry_linking: false,
            disallowed_new_abilities: AbilitySet::EMPTY,
        }
        .check(&module, &updated_module)
        .is_ok()
    );

    // allow updating signatures of private entry functions
    assert!(
        Compatibility {
            check_datatype_layout: true,
            check_private_entry_linking: false,
            disallowed_new_abilities: AbilitySet::EMPTY,
        }
        .check(&updated_module, &module)
        .is_ok()
    );

    // disallow updating signatures of private entry functions
    assert!(
        Compatibility::full_check()
            .check(&module, &updated_module)
            .is_err()
    );
}

#[test]
fn entry_fun_compat_tests() {
    let pool = &mut RcPool::new();
    // fun
    let private_fun = max_version(mk_module_entry(pool, Visibility::Private as u8, false));
    // entry fun
    let entry_fun = max_version(mk_module_entry(pool, Visibility::Private as u8, true));
    // public(friend) fun
    let friend_fun = max_version(mk_module_entry(pool, Visibility::Friend as u8, false));
    // public(friend) entry fun
    let friend_entry_fun = max_version(mk_module_entry(pool, Visibility::Friend as u8, true));
    // public fun
    let public_fun = max_version(mk_module_entry(pool, Visibility::Public as u8, false));
    // public entry fun
    let public_entry_fun = max_version(mk_module_entry(pool, Visibility::Public as u8, true));
    // NO function
    let mut no_fun = private_fun.clone();
    no_fun.functions.clear();

    let mut valid_combos = vec![
        // Can transition from a private entry fun to anything
        // see the adding of `invalid_private_entry_breakages` below to this table.
        (&entry_fun, &friend_entry_fun),
        // Can transition from a private fun to anything
        (&private_fun, &entry_fun),
        (&private_fun, &friend_fun),
        (&private_fun, &friend_entry_fun),
        (&private_fun, &public_fun),
        (&private_fun, &public_entry_fun),
        (&private_fun, &no_fun),
        // Can transition from public fun to a public entry fun (post version 5)
        (&public_fun, &public_entry_fun),
        (&friend_entry_fun, &public_entry_fun),
        (&friend_entry_fun, &friend_fun),
        (&friend_fun, &public_entry_fun),
        (&friend_fun, &public_fun),
        (&friend_fun, &friend_entry_fun),
        (&public_entry_fun, &public_fun),
    ];

    let invalid_combos = vec![
        (&public_entry_fun, &entry_fun),
        (&public_entry_fun, &private_fun),
        (&public_entry_fun, &friend_entry_fun),
        (&public_entry_fun, &friend_fun),
        (&public_entry_fun, &no_fun),
    ];

    let invalid_private_entry_breakages = vec![
        (&entry_fun, &private_fun),
        (&entry_fun, &friend_fun),
        (&entry_fun, &public_fun),
        (&entry_fun, &no_fun),
    ];

    valid_combos.extend_from_slice(&invalid_private_entry_breakages);

    // These would all be invalid combos unless we also allow friend breakage.
    let valid_entry_fun_changes_with_friend_api_breakage = vec![
        (&friend_entry_fun, &no_fun),
        (&friend_entry_fun, &private_fun),
        (&friend_entry_fun, &entry_fun),
        (&friend_fun, &no_fun),
        (&friend_fun, &private_fun),
        (&friend_fun, &entry_fun),
    ];

    // Every valid combo is valid under `check_private_entry_linking = false`
    for (prev, new) in valid_combos.into_iter() {
        assert!(
            Compatibility {
                check_datatype_layout: true,
                check_private_entry_linking: false,
                disallowed_new_abilities: AbilitySet::EMPTY,
            }
            .check(prev, new)
            .is_ok()
        );
    }

    // Every
    for (prev, new) in invalid_private_entry_breakages.into_iter() {
        assert!(Compatibility::full_check().check(prev, new).is_err());
    }

    // Every valid combo is valid under `check_private_entry_linking = false`
    for (prev, new) in valid_entry_fun_changes_with_friend_api_breakage.into_iter() {
        assert!(
            Compatibility {
                check_datatype_layout: true,
                check_private_entry_linking: false,
                disallowed_new_abilities: AbilitySet::EMPTY,
            }
            .check(prev, new)
            .is_ok()
        );
    }

    for (prev, new) in invalid_combos.into_iter() {
        assert!(
            Compatibility {
                check_datatype_layout: true,
                check_private_entry_linking: false,
                disallowed_new_abilities: AbilitySet::EMPTY,
            }
            .check(prev, new)
            .is_err()
        );
    }
}

#[test]
fn public_entry_signature_change_disallowed() {
    let pool = &mut RcPool::new();
    // Create a public entry function
    let module = max_version(mk_module_entry(pool, Visibility::Public as u8, true));
    let mut updated_module = module.clone();
    let function_ref = updated_module.functions.get_mut(ident_str!("fn")).unwrap();
    let new_function = {
        let mut f: normalized::Function<_> = (**function_ref).clone();
        f.parameters = Rc::new(vec![Rc::new(Type::U64)]);
        Rc::new(f)
    };
    *function_ref = new_function;
    // Update the signature of the entry fun to now take a u64 argument.

    assert!(
        Compatibility {
            check_datatype_layout: true,
            check_private_entry_linking: false,
            disallowed_new_abilities: AbilitySet::EMPTY,
        }
        .check(&module, &updated_module)
        .is_err()
    );

    assert!(
        Compatibility {
            check_datatype_layout: true,
            check_private_entry_linking: false,
            disallowed_new_abilities: AbilitySet::EMPTY,
        }
        .check(&updated_module, &module)
        .is_err()
    );

    assert!(
        Compatibility {
            check_datatype_layout: true,
            check_private_entry_linking: true,
            disallowed_new_abilities: AbilitySet::EMPTY,
        }
        .check(&module, &updated_module)
        .is_err()
    );
}

#[test]
fn friend_entry_signature_change_allowed() {
    let pool = &mut RcPool::new();
    let module = max_version(mk_module_entry(pool, Visibility::Friend as u8, true));
    let mut updated_module = module.clone();
    // Update the signature of the entry fun to now take a u64 argument.
    let function_ref = updated_module.functions.get_mut(ident_str!("fn")).unwrap();
    let new_function = {
        let mut f: normalized::Function<_> = (**function_ref).clone();
        f.parameters = Rc::new(vec![Rc::new(Type::U64)]);
        Rc::new(f)
    };
    *function_ref = new_function;

    assert!(
        Compatibility {
            check_datatype_layout: true,
            check_private_entry_linking: false,
            disallowed_new_abilities: AbilitySet::EMPTY,
        }
        .check(&module, &updated_module)
        .is_ok()
    );

    assert!(
        Compatibility {
            check_datatype_layout: true,
            check_private_entry_linking: true,
            disallowed_new_abilities: AbilitySet::EMPTY,
        }
        .check(&module, &updated_module)
        .is_err()
    );
}

#[test]
fn check_exact_and_unchange_same_module() {
    let pool = &mut RcPool::new();
    let m1 = max_version(mk_module(pool, Visibility::Private as u8));
    assert!(InclusionCheck::Subset.check(&m1, &m1).is_ok());
    assert!(InclusionCheck::Equal.check(&m1, &m1).is_ok());

    // m1 + an extra function
    let m2 = max_version(mk_module_plus(pool, Visibility::Private as u8));
    assert!(InclusionCheck::Subset.check(&m1, &m2).is_ok());
    assert!(InclusionCheck::Subset.check(&m2, &m1).is_err());
    assert!(InclusionCheck::Equal.check(&m1, &m2).is_err());
    assert!(InclusionCheck::Equal.check(&m2, &m1).is_err());

    // m1 + a change in the bytecode of fn
    let m3 = max_version(mk_module_plus_code(
        pool,
        Visibility::Private as u8,
        vec![Bytecode::LdU8(0), Bytecode::Ret],
    ));
    assert!(InclusionCheck::Subset.check(&m2, &m3).is_err());
    // fn1 is not in m1 so the changed bytecode doesn't matter.
    assert!(InclusionCheck::Subset.check(&m1, &m3).is_ok());
    assert!(InclusionCheck::Subset.check(&m3, &m2).is_err());
    assert!(InclusionCheck::Subset.check(&m3, &m1).is_err());
    assert!(InclusionCheck::Equal.check(&m1, &m3).is_err());
    assert!(InclusionCheck::Equal.check(&m2, &m3).is_err());
}

#[test]
fn check_exact_and_unchange_same_module_permutations() {
    let pool = &mut RcPool::new();
    let m1 = max_version(mk_module(pool, Visibility::Private as u8));
    let m2 = max_version(mk_module_plus(pool, Visibility::Private as u8));
    let m3 = max_version(mk_module_plus_perm(
        pool,
        Visibility::Private as u8,
        Permutation::new(vec![1, 0]),
    ));
    assert!(!m2.equivalent(&m3));
    assert!(InclusionCheck::Equal.check(&m2, &m3).is_ok());
    assert!(InclusionCheck::Equal.check(&m3, &m2).is_ok());
    // double inclusion
    assert!(InclusionCheck::Subset.check(&m3, &m2).is_ok());
    assert!(InclusionCheck::Subset.check(&m2, &m3).is_ok());

    assert!(InclusionCheck::Subset.check(&m1, &m3).is_ok());
    assert!(InclusionCheck::Subset.check(&m3, &m1).is_err());
    assert!(InclusionCheck::Equal.check(&m1, &m3).is_err());
    assert!(InclusionCheck::Equal.check(&m3, &m1).is_err());
}

#[test]
fn check_exact_and_unchange_same_complex_module_permutations() {
    let pool = &mut RcPool::new();
    let perms = vec![
        vec![0, 1, 2],
        vec![0, 2, 1],
        vec![1, 0, 2],
        vec![1, 2, 0],
        vec![2, 0, 1],
        vec![2, 1, 0],
    ];
    let modules: Vec<_> = perms
        .into_iter()
        .map(|permutation| {
            max_version(make_complex_module_perm(
                pool,
                Permutation::new(permutation),
            ))
        })
        .collect();

    for m0 in &modules {
        for m1 in &modules {
            assert!(InclusionCheck::Equal.check(m0, m1).is_ok());
            assert!(InclusionCheck::Equal.check(m1, m0).is_ok());
            assert!(InclusionCheck::Subset.check(m0, m1).is_ok());
            assert!(InclusionCheck::Subset.check(m1, m0).is_ok());
        }
    }
}

#[test]
fn check_new_changed_missing_declarations() {
    let pool = &mut RcPool::new();
    let empty = mk_module_with_defs(pool, vec![], vec![], vec![], vec![]);
    // struct
    let m1 = mk_module_with_defs(
        pool,
        vec![(
            Identifier::new("S1").unwrap(),
            StructDefinition {
                struct_handle: DatatypeHandleIndex(0),
                field_information: StructFieldInformation::Declared(vec![FieldDefinition {
                    name: IdentifierIndex(1),
                    signature: TypeSignature(SignatureToken::U64),
                }]),
            },
        )],
        vec![],
        vec![],
        vec![],
    );

    // change struct S1 to S2
    let m2 = mk_module_with_defs(
        pool,
        vec![(
            Identifier::new("S2").unwrap(),
            StructDefinition {
                struct_handle: DatatypeHandleIndex(0),
                field_information: StructFieldInformation::Declared(vec![FieldDefinition {
                    name: IdentifierIndex(1),
                    signature: TypeSignature(SignatureToken::U64),
                }]),
            },
        )],
        vec![],
        vec![],
        vec![],
    );

    // same name different type
    let m3 = mk_module_with_defs(
        pool,
        vec![(
            Identifier::new("S1").unwrap(),
            StructDefinition {
                struct_handle: DatatypeHandleIndex(0),
                field_information: StructFieldInformation::Declared(vec![FieldDefinition {
                    name: IdentifierIndex(1),
                    // changed to U32
                    signature: TypeSignature(SignatureToken::U32),
                }]),
            },
        )],
        vec![],
        vec![],
        vec![],
    );

    assert!(InclusionCheck::Subset.check(&m1, &m1).is_ok());
    assert!(InclusionCheck::Subset.check(&m1, &empty).is_err());
    assert!(InclusionCheck::Subset.check(&m1, &m2).is_err());
    assert!(InclusionCheck::Subset.check(&m1, &m3).is_err());

    assert!(InclusionCheck::Subset.check(&empty, &m1).is_ok());
    assert!(InclusionCheck::Equal.check(&empty, &m1).is_err());

    //enums
    let m1 = mk_module_with_defs(
        pool,
        vec![],
        vec![(
            Identifier::new("E1").unwrap(),
            EnumDefinition {
                enum_handle: DatatypeHandleIndex(0),
                variants: vec![VariantDefinition {
                    variant_name: IdentifierIndex(1),
                    fields: vec![FieldDefinition {
                        name: IdentifierIndex(1),
                        signature: TypeSignature(SignatureToken::U64),
                    }],
                }],
            },
        )],
        vec![],
        vec![],
    );

    // change enum E1 to E2
    let m2 = mk_module_with_defs(
        pool,
        vec![],
        vec![(
            Identifier::new("E2").unwrap(),
            EnumDefinition {
                enum_handle: DatatypeHandleIndex(0),
                variants: vec![VariantDefinition {
                    variant_name: IdentifierIndex(1),
                    fields: vec![FieldDefinition {
                        name: IdentifierIndex(1),
                        signature: TypeSignature(SignatureToken::U64),
                    }],
                }],
            },
        )],
        vec![],
        vec![],
    );

    // same name different type
    let m3 = mk_module_with_defs(
        pool,
        vec![],
        vec![(
            Identifier::new("E1").unwrap(),
            EnumDefinition {
                enum_handle: DatatypeHandleIndex(0),
                variants: vec![VariantDefinition {
                    variant_name: IdentifierIndex(1),
                    fields: vec![FieldDefinition {
                        name: IdentifierIndex(1),
                        // changed to U32
                        signature: TypeSignature(SignatureToken::U32),
                    }],
                }],
            },
        )],
        vec![],
        vec![],
    );

    assert!(InclusionCheck::Subset.check(&m1, &m1).is_ok());
    assert!(InclusionCheck::Equal.check(&m1, &m1).is_ok());

    assert!(InclusionCheck::Subset.check(&m1, &empty).is_err());
    assert!(InclusionCheck::Subset.check(&m1, &m2).is_err());
    assert!(InclusionCheck::Subset.check(&m1, &m3).is_err());

    assert!(InclusionCheck::Subset.check(&empty, &m1).is_ok());
    assert!(InclusionCheck::Equal.check(&empty, &m1).is_err());

    //functions
    let m1 = mk_module_with_defs(
        pool,
        vec![],
        vec![],
        vec![(
            Identifier::new("fn1").unwrap(),
            FunctionDefinition {
                function: FunctionHandleIndex(0),
                visibility: Visibility::Public,
                is_entry: false,
                acquires_global_resources: vec![],
                code: Some(CodeUnit {
                    locals: SignatureIndex(0),
                    code: vec![Bytecode::Ret],
                    jump_tables: vec![],
                }),
            },
        )],
        vec![],
    );

    // change function fn1 to fn2
    let m2 = mk_module_with_defs(
        pool,
        vec![],
        vec![],
        vec![(
            Identifier::new("fn2").unwrap(),
            FunctionDefinition {
                function: FunctionHandleIndex(0),
                visibility: Visibility::Public,
                is_entry: false,
                acquires_global_resources: vec![],
                code: Some(CodeUnit {
                    locals: SignatureIndex(0),
                    code: vec![Bytecode::Ret],
                    jump_tables: vec![],
                }),
            },
        )],
        vec![],
    );

    // change fn1 bytecode to abort from return
    let m3 = mk_module_with_defs(
        pool,
        vec![],
        vec![],
        vec![(
            Identifier::new("fn1").unwrap(),
            FunctionDefinition {
                function: FunctionHandleIndex(0),
                visibility: Visibility::Public,
                is_entry: false,
                acquires_global_resources: vec![],
                code: Some(CodeUnit {
                    locals: SignatureIndex(0),
                    code: vec![Bytecode::Abort],
                    jump_tables: vec![],
                }),
            },
        )],
        vec![],
    );

    assert!(InclusionCheck::Subset.check(&m1, &m1).is_ok());
    assert!(InclusionCheck::Equal.check(&m1, &m1).is_ok());

    assert!(InclusionCheck::Subset.check(&m1, &empty).is_err());
    assert!(InclusionCheck::Subset.check(&m1, &m2).is_err());
    assert!(InclusionCheck::Subset.check(&m1, &m3).is_err());

    assert!(InclusionCheck::Subset.check(&empty, &m1).is_ok());
    assert!(InclusionCheck::Equal.check(&empty, &m1).is_err());
}

#[test]
fn test_friend_linking() {
    let pool = &mut RcPool::new();
    let friend_modules = [
        (Identifier::new("M1").unwrap(), AccountAddress::random()),
        (Identifier::new("M2").unwrap(), AccountAddress::random()),
        (Identifier::new("M3").unwrap(), AccountAddress::random()),
        (Identifier::new("M4").unwrap(), AccountAddress::random()),
    ];

    // zero friends
    let m0 = mk_module_with_defs(pool, vec![], vec![], vec![], vec![]);

    // two friends
    let m1 = mk_module_with_defs(
        pool,
        vec![],
        vec![],
        vec![],
        vec![friend_modules[0].clone(), friend_modules[1].clone()],
    );

    // 3 friends
    let m2 = mk_module_with_defs(
        pool,
        vec![],
        vec![],
        vec![],
        vec![
            friend_modules[0].clone(),
            friend_modules[1].clone(),
            friend_modules[2].clone(),
        ],
    );

    // 2 friends from m1, but different order
    let m3 = mk_module_with_defs(
        pool,
        vec![],
        vec![],
        vec![],
        vec![friend_modules[1].clone(), friend_modules[0].clone()],
    );

    // 2 friends, different from m1
    let m4 = mk_module_with_defs(
        pool,
        vec![],
        vec![],
        vec![],
        vec![friend_modules[2].clone(), friend_modules[3].clone()],
    );

    // Subset, all changes are allowed
    // same module, no friends
    assert!(InclusionCheck::Subset.check(&m0, &m0).is_ok());
    // start with empty and add friend
    assert!(InclusionCheck::Subset.check(&m0, &m1).is_ok());
    // start with two, keep the two and add one more
    assert!(InclusionCheck::Subset.check(&m1, &m2).is_ok());
    // start with two, remove them
    assert!(InclusionCheck::Subset.check(&m3, &m0).is_ok());
    // start with three remove one
    assert!(InclusionCheck::Subset.check(&m2, &m1).is_ok());
    // change order
    assert!(InclusionCheck::Subset.check(&m1, &m3).is_ok());
    // 2 friends, changed to 2 different friends
    assert!(InclusionCheck::Subset.check(&m1, &m4).is_ok());

    // Equal, can only keep the same number of friends
    // same module, no friends
    assert!(InclusionCheck::Equal.check(&m0, &m0).is_ok());
    // start with empty and add friends
    assert!(InclusionCheck::Equal.check(&m0, &m1).is_err());
    // start with two, keep the two and add one more
    assert!(InclusionCheck::Equal.check(&m1, &m2).is_err());
    // start with two, remove them
    assert!(InclusionCheck::Equal.check(&m3, &m0).is_err());
    // start with three remove one
    assert!(InclusionCheck::Equal.check(&m2, &m1).is_err());
    // change order
    assert!(InclusionCheck::Equal.check(&m1, &m3).is_ok());
    // 2 friends, changed to 2 different friends
    assert!(InclusionCheck::Equal.check(&m1, &m4).is_ok());
}

#[test]
fn test_compare_ord_iters() {
    let a = Identifier::new("a").unwrap();
    let b = Identifier::new("b").unwrap();
    let c = Identifier::new("c").unwrap();
    let d = Identifier::new("d").unwrap();
    let e = Identifier::new("e").unwrap();
    let f = Identifier::new("f").unwrap();

    // (old, new, expected, name)
    let tests = vec![
        ("empty", vec![], vec![], vec![]),
        (
            "existing",
            vec![&a],
            vec![&a],
            vec![Mark::Existing(&a, &(), &())],
        ),
        ("missing", vec![&a], vec![], vec![Mark::Missing(&a, &())]),
        ("new", vec![], vec![&a], vec![Mark::New(&a, &())]),
        (
            "missing new",
            vec![&a],
            vec![&b],
            vec![Mark::Missing(&a, &()), Mark::New(&b, &())],
        ),
        (
            "existing new",
            vec![&a],
            vec![&a, &b],
            vec![Mark::Existing(&a, &(), &()), Mark::New(&b, &())],
        ),
        (
            "existing missing",
            vec![&a, &b],
            vec![&a],
            vec![Mark::Existing(&a, &(), &()), Mark::Missing(&b, &())],
        ),
        (
            "missing existing",
            vec![&a, &b],
            vec![&b],
            vec![Mark::Missing(&a, &()), Mark::Existing(&b, &(), &())],
        ),
        (
            "existing new missing",
            vec![&a, &e],
            vec![&a, &b],
            vec![
                Mark::Existing(&a, &(), &()),
                Mark::New(&b, &()),
                Mark::Missing(&e, &()),
            ],
        ),
        (
            "new existing missing",
            vec![&b, &f],
            vec![&a, &b],
            vec![
                Mark::New(&a, &()),
                Mark::Existing(&b, &(), &()),
                Mark::Missing(&f, &()),
            ],
        ),
        (
            "new missing existing",
            vec![&d, &e],
            vec![&a, &e],
            vec![
                Mark::New(&a, &()),
                Mark::Missing(&d, &()),
                Mark::Existing(&e, &(), &()),
            ],
        ),
        (
            "missing existing new",
            vec![&a, &b],
            vec![&b, &c],
            vec![
                Mark::Missing(&a, &()),
                Mark::Existing(&b, &(), &()),
                Mark::New(&c, &()),
            ],
        ),
    ];

    for (name, old, new, expected) in tests {
        let old = old.into_iter().map(|x| (x, &())).collect::<Vec<_>>();
        let new = new.into_iter().map(|x| (x, &())).collect::<Vec<_>>();

        let iter = compare_ord_iters(old.into_iter(), new.into_iter());
        let result: Vec<_> = iter.collect();
        assert_eq!(
            result, expected,
            "failed: {}. Result: {:?}, Expected: {:?}",
            name, result, expected
        );
    }
}

fn arbitrary_map() -> impl Strategy<Value = BTreeMap<i32, i32>> {
    prop::collection::btree_map(0..1000, 0..1000, 0..20)
}

proptest! {
    #[test]
    fn proptest_compare_ord_iters(old in arbitrary_map(), new in arbitrary_map()) {
        let result: Vec<_> = compare_ord_iters(old.iter(), new.iter()).collect();

        // For each 'mark' make sure the key exists in the expected maps of old, new or both
        for mark in &result {
            match mark {
                Mark::New(key, _) => {
                    // New keys must only be in `new`
                    prop_assert!(new.contains_key(key));
                    prop_assert!(!old.contains_key(key));
                },
                Mark::Missing(key, _) => {
                    // Missing keys must only be in `old`
                    prop_assert!(old.contains_key(key));
                    prop_assert!(!new.contains_key(key));
                },
                Mark::Existing(key, old_value, new_value) => {
                    let old_expected = old.get(key).expect("key exists in old");
                    let new_expected = new.get(key).expect("key exists in new");
                    // Existing keys must be in both
                    prop_assert!(old.contains_key(key));
                    prop_assert!(new.contains_key(key));
                    // Values must match the input maps
                    prop_assert_eq!(Some(*old_value), Some(old_expected));
                    prop_assert_eq!(Some(*new_value), Some(new_expected));
                },
            }
        }

        // check that the combined keys of old and new are the same as the keys in result
        // btreeset is used to dedup 'existing' keys
        let combined: BTreeSet<_> = old.keys().chain(new.keys()).collect();

        let result_keys: BTreeSet<_> = result
            .iter()
            .map(|mark| match mark {
                Mark::New(key, _) | Mark::Missing(key, _) | Mark::Existing(key, _, _) => *key,
            })
            .collect();

        prop_assert_eq!(result_keys, combined);
    }
}
