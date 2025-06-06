// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use anyhow::anyhow;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    str::FromStr,
};

use crate::lexer::*;
use move_command_line_common::files::FileHash;
use move_core_types::{account_address::AccountAddress, u256};
use move_ir_types::{ast::*, location::*};
use move_symbol_pool::Symbol;

// FIXME: The following simplified version of ParseError copied from
// lalrpop-util should be replaced.

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ParseError<L, E> {
    InvalidToken { location: L, message: String },
    User { location: L, error: E },
}

impl<L, E> fmt::Display for ParseError<L, E>
where
    L: fmt::Display,
    E: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::ParseError::*;
        match *self {
            User { ref error, .. } => write!(f, "{}", error),
            InvalidToken {
                ref location,
                ref message,
            } => write!(f, "Invalid token at {}: {}", location, message),
        }
    }
}

fn make_loc(file_hash: FileHash, start: usize, end: usize) -> Loc {
    Loc::new(file_hash, start as u32, end as u32)
}

fn current_token_loc(tokens: &Lexer) -> Loc {
    let start_loc = tokens.start_loc();
    make_loc(
        tokens.file_hash(),
        start_loc,
        start_loc + tokens.content().len(),
    )
}

fn spanned<T>(file_hash: FileHash, start: usize, end: usize, value: T) -> Spanned<T> {
    Spanned {
        loc: make_loc(file_hash, start, end),
        value,
    }
}

// Check for the specified token and consume it if it matches.
// Returns true if the token matches.
fn match_token(tokens: &mut Lexer, tok: Tok) -> Result<bool, ParseError<Loc, anyhow::Error>> {
    if tokens.peek() == tok {
        tokens.advance()?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn consume_token(tokens: &mut Lexer, tok: Tok) -> Result<(), ParseError<Loc, anyhow::Error>> {
    if tokens.peek() != tok {
        return Err(ParseError::InvalidToken {
            location: current_token_loc(tokens),
            message: format!("expected {:?}, not {:?}", tok, tokens.peek()),
        });
    }
    tokens.advance()?;
    Ok(())
}

fn adjust_token(
    tokens: &mut Lexer,
    list_end_tokens: &[Tok],
) -> Result<(), ParseError<Loc, anyhow::Error>> {
    if tokens.peek() == Tok::GreaterGreater && list_end_tokens.contains(&Tok::Greater) {
        tokens.replace_token(Tok::Greater, 1)?;
    }
    Ok(())
}

fn parse_comma_list<F, R>(
    tokens: &mut Lexer,
    list_end_tokens: &[Tok],
    parse_list_item: F,
    allow_trailing_comma: bool,
) -> Result<Vec<R>, ParseError<Loc, anyhow::Error>>
where
    F: Fn(&mut Lexer) -> Result<R, ParseError<Loc, anyhow::Error>>,
{
    let mut v = vec![];
    adjust_token(tokens, list_end_tokens)?;
    if !list_end_tokens.contains(&tokens.peek()) {
        loop {
            v.push(parse_list_item(tokens)?);
            adjust_token(tokens, list_end_tokens)?;
            if list_end_tokens.contains(&tokens.peek()) {
                break;
            }
            consume_token(tokens, Tok::Comma)?;
            adjust_token(tokens, list_end_tokens)?;
            if list_end_tokens.contains(&tokens.peek()) && allow_trailing_comma {
                break;
            }
        }
    }
    Ok(v)
}

fn parse_list<C, F, R>(
    tokens: &mut Lexer,
    mut parse_list_continue: C,
    parse_list_item: F,
) -> Result<Vec<R>, ParseError<Loc, anyhow::Error>>
where
    C: FnMut(&mut Lexer) -> Result<bool, ParseError<Loc, anyhow::Error>>,
    F: Fn(&mut Lexer) -> Result<R, ParseError<Loc, anyhow::Error>>,
{
    let mut v = vec![];
    loop {
        v.push(parse_list_item(tokens)?);
        if !parse_list_continue(tokens)? {
            break Ok(v);
        }
    }
}

fn parse_name(tokens: &mut Lexer) -> Result<Symbol, ParseError<Loc, anyhow::Error>> {
    if tokens.peek() != Tok::NameValue {
        return Err(ParseError::InvalidToken {
            location: current_token_loc(tokens),
            message: "expected Tok::NameValue".to_string(),
        });
    }
    let name = tokens.content();
    tokens.advance()?;
    Ok(Symbol::from(name))
}

fn parse_name_begin_ty(tokens: &mut Lexer) -> Result<Symbol, ParseError<Loc, anyhow::Error>> {
    if tokens.peek() != Tok::NameBeginTyValue {
        return Err(ParseError::InvalidToken {
            location: current_token_loc(tokens),
            message: "expected Tok::NameBeginTyValue".to_string(),
        });
    }
    let s = tokens.content();
    // The token includes a "<" at the end, so chop that off to get the name.
    let name = &s[..s.len() - 1];
    tokens.advance()?;
    Ok(Symbol::from(name))
}

fn parse_dot_name<'input>(
    tokens: &mut Lexer<'input>,
) -> Result<&'input str, ParseError<Loc, anyhow::Error>> {
    if tokens.peek() != Tok::DotNameValue {
        return Err(ParseError::InvalidToken {
            location: current_token_loc(tokens),
            message: "expected Tok::DotNameValue".to_string(),
        });
    }
    let name = tokens.content();
    tokens.advance()?;
    Ok(name)
}

// AccountAddress: AccountAddress = {
//     < s: r"0[xX][0-9a-fA-F]+" > => { ... }
// };

fn parse_account_address(
    tokens: &mut Lexer,
) -> Result<AccountAddress, ParseError<Loc, anyhow::Error>> {
    if !matches!(tokens.peek(), Tok::AccountAddressValue | Tok::NameValue) {
        return Err(ParseError::InvalidToken {
            location: current_token_loc(tokens),
            message: "expected Tok::AccountAddressValue".to_string(),
        });
    }
    let loc = current_token_loc(tokens);
    let addr = parse_address_literal(tokens, tokens.content(), loc).unwrap();
    tokens.advance()?;
    Ok(addr)
}

fn parse_address_literal(
    lexer: &Lexer,
    literal: &str,
    location: Loc,
) -> Result<AccountAddress, ParseError<Loc, anyhow::Error>> {
    let Some(addr) = AccountAddress::from_hex_literal(literal)
        .ok()
        .or_else(|| lexer.resolve_named_address(literal))
    else {
        return Err(ParseError::InvalidToken {
            location,
            message: format!("Invalid address '{}'", literal),
        });
    };
    Ok(addr)
}

// Var: Var = {
//     <n:Name> =>? Var::parse(n),
// };

fn parse_var_(tokens: &mut Lexer) -> Result<Var_, ParseError<Loc, anyhow::Error>> {
    Ok(Var_(parse_name(tokens)?))
}

fn parse_var(tokens: &mut Lexer) -> Result<Var, ParseError<Loc, anyhow::Error>> {
    let start_loc = tokens.start_loc();
    let var = parse_var_(tokens)?;
    let end_loc = tokens.previous_end_loc();
    Ok(spanned(tokens.file_hash(), start_loc, end_loc, var))
}

// Field: Field = {
//     <n:Name> =>? parse_field(n),
// };

fn parse_field(tokens: &mut Lexer) -> Result<Field, ParseError<Loc, anyhow::Error>> {
    let start_loc = tokens.start_loc();
    let f = Field_(parse_name(tokens)?);
    let end_loc = tokens.previous_end_loc();
    Ok(spanned(tokens.file_hash(), start_loc, end_loc, f))
}

/// field-ident: name-and-type-actuals '::' field
fn parse_field_ident(tokens: &mut Lexer) -> Result<FieldIdent, ParseError<Loc, anyhow::Error>> {
    let start_loc = tokens.start_loc();
    let (name, type_actuals) = parse_name_and_type_actuals(tokens)?;
    // For now, the lexer produces 2 ':' tokens instead of a single '::' token.
    consume_token(tokens, Tok::Colon)?;
    consume_token(tokens, Tok::Colon)?;
    let field = parse_field(tokens)?;
    let end_loc = tokens.previous_end_loc();
    Ok(spanned(
        tokens.file_hash(),
        start_loc,
        end_loc,
        FieldIdent_ {
            struct_name: DatatypeName(name),
            type_actuals,
            field,
        },
    ))
}

// CopyableVal: CopyableVal = {
//     AccountAddress => CopyableVal::Address(<>),
//     "true" => CopyableVal::Bool(true),
//     "false" => CopyableVal::Bool(false),
//     <i: U64> => CopyableVal::U64(i),
//     <buf: ByteArray> => CopyableVal::ByteArray(buf),
// }

fn parse_copyable_val(tokens: &mut Lexer) -> Result<CopyableVal, ParseError<Loc, anyhow::Error>> {
    let start_loc = tokens.start_loc();
    let val = match tokens.peek() {
        Tok::AccountAddressValue => {
            let addr = parse_account_address(tokens)?;
            CopyableVal_::Address(addr)
        }
        Tok::True => {
            tokens.advance()?;
            CopyableVal_::Bool(true)
        }
        Tok::False => {
            tokens.advance()?;
            CopyableVal_::Bool(false)
        }
        Tok::U8Value => {
            let mut s = tokens.content();
            if s.ends_with("u8") {
                s = &s[..s.len() - 2]
            }
            let i = u8::from_str(s).unwrap();
            tokens.advance()?;
            CopyableVal_::U8(i)
        }
        Tok::U16Value => {
            let mut s = tokens.content();
            if s.ends_with("u16") {
                s = &s[..s.len() - 3]
            }
            let i = u16::from_str(s).unwrap();
            tokens.advance()?;
            CopyableVal_::U16(i)
        }
        Tok::U32Value => {
            let mut s = tokens.content();
            if s.ends_with("u32") {
                s = &s[..s.len() - 3]
            }
            let i = u32::from_str(s).unwrap();
            tokens.advance()?;
            CopyableVal_::U32(i)
        }
        Tok::U64Value => {
            let mut s = tokens.content();
            if s.ends_with("u64") {
                s = &s[..s.len() - 3]
            }
            let i = u64::from_str(s).unwrap();
            tokens.advance()?;
            CopyableVal_::U64(i)
        }
        Tok::U128Value => {
            let mut s = tokens.content();
            if s.ends_with("u128") {
                s = &s[..s.len() - 4]
            }
            let i = u128::from_str(s).unwrap();
            tokens.advance()?;
            CopyableVal_::U128(i)
        }
        Tok::U256Value => {
            let mut s = tokens.content();
            if s.ends_with("256") {
                s = &s[..s.len() - 4]
            }
            let i = u256::U256::from_str(s).unwrap();
            tokens.advance()?;
            CopyableVal_::U256(i)
        }
        Tok::ByteArrayValue => {
            let s = tokens.content();
            let buf = hex::decode(&s[2..s.len() - 1]).unwrap_or_else(|_| {
                // The lexer guarantees this, but tracking this knowledge all the way to here is tedious
                unreachable!("The string {:?} is not a valid hex-encoded byte array", s)
            });
            tokens.advance()?;
            CopyableVal_::ByteArray(buf)
        }
        t => {
            return Err(ParseError::InvalidToken {
                location: current_token_loc(tokens),
                message: format!("unrecognized token kind {:?}", t),
            });
        }
    };
    let end_loc = tokens.previous_end_loc();
    Ok(spanned(tokens.file_hash(), start_loc, end_loc, val))
}

// Get the precedence of a binary operator. The minimum precedence value
// is 1, and larger values have higher precedence. For tokens that are not
// binary operators, this returns a value of zero so that they will be
// below the minimum value and will mark the end of the binary expression
// for the code in parse_rhs_of_binary_exp.
// Precedences are not sequential to make it easier to add new binops without
// renumbering everything.
fn get_precedence(token: Tok) -> u32 {
    match token {
        // Reserved minimum precedence value is 1 (specified in parse_exp_)
        // TODO
        // Tok::EqualEqualGreater may not work right,
        // since parse_spec_exp calls parse_rhs_of_spec_exp
        // with min_prec = 1.  So parse_spec_expr will stop parsing instead of reading ==>
        Tok::EqualEqualGreater => 1,
        Tok::PipePipe => 5,
        Tok::AmpAmp => 10,
        Tok::EqualEqual => 15,
        Tok::ExclaimEqual => 15,
        Tok::Less => 15,
        Tok::Greater => 15,
        Tok::LessEqual => 15,
        Tok::GreaterEqual => 15,
        Tok::Pipe => 25,
        Tok::Caret => 30,
        Tok::Amp => 35,
        Tok::LessLess => 40,
        Tok::GreaterGreater => 40,
        Tok::Plus => 45,
        Tok::Minus => 45,
        Tok::Star => 50,
        Tok::Slash => 50,
        Tok::Percent => 50,
        _ => 0, // anything else is not a binary operator
    }
}

fn parse_exp(tokens: &mut Lexer) -> Result<Exp, ParseError<Loc, anyhow::Error>> {
    let lhs = parse_unary_exp(tokens)?;
    parse_rhs_of_binary_exp(tokens, lhs, /* min_prec */ 1)
}

fn parse_rhs_of_binary_exp(
    tokens: &mut Lexer,
    lhs: Exp,
    min_prec: u32,
) -> Result<Exp, ParseError<Loc, anyhow::Error>> {
    let mut result = lhs;
    let mut next_tok_prec = get_precedence(tokens.peek());

    // Continue parsing binary expressions as long as they have they
    // specified minimum precedence.
    while next_tok_prec >= min_prec {
        let op_token = tokens.peek();
        tokens.advance()?;

        let mut rhs = parse_unary_exp(tokens)?;

        // If the next token is another binary operator with a higher
        // precedence, then recursively parse that expression as the RHS.
        let this_prec = next_tok_prec;
        next_tok_prec = get_precedence(tokens.peek());
        if this_prec < next_tok_prec {
            rhs = parse_rhs_of_binary_exp(tokens, rhs, this_prec + 1)?;
            next_tok_prec = get_precedence(tokens.peek());
        }

        let op = match op_token {
            Tok::EqualEqual => BinOp::Eq,
            Tok::ExclaimEqual => BinOp::Neq,
            Tok::Less => BinOp::Lt,
            Tok::Greater => BinOp::Gt,
            Tok::LessEqual => BinOp::Le,
            Tok::GreaterEqual => BinOp::Ge,
            Tok::PipePipe => BinOp::Or,
            Tok::AmpAmp => BinOp::And,
            Tok::Caret => BinOp::Xor,
            Tok::LessLess => BinOp::Shl,
            Tok::GreaterGreater => BinOp::Shr,
            Tok::Pipe => BinOp::BitOr,
            Tok::Amp => BinOp::BitAnd,
            Tok::Plus => BinOp::Add,
            Tok::Minus => BinOp::Sub,
            Tok::Star => BinOp::Mul,
            Tok::Slash => BinOp::Div,
            Tok::Percent => BinOp::Mod,
            _ => panic!("Unexpected token that is not a binary operator"),
        };
        let start_loc = result.loc.start();
        let end_loc = tokens.previous_end_loc();
        let e = Exp_::BinopExp(Box::new(result), op, Box::new(rhs));
        result = spanned(tokens.file_hash(), start_loc as usize, end_loc, e);
    }

    Ok(result)
}

// QualifiedFunctionName : FunctionCall = {
//     <f: Builtin> => FunctionCall::Builtin(f),
//     <module_dot_name: DotName> <type_actuals: TypeActuals> =>? { ... }
// }

fn parse_qualified_function_name(
    tokens: &mut Lexer,
) -> Result<FunctionCall, ParseError<Loc, anyhow::Error>> {
    let start_loc = tokens.start_loc();
    let call = match tokens.peek() {
        Tok::VecPack(_)
        | Tok::VecLen
        | Tok::VecImmBorrow
        | Tok::VecMutBorrow
        | Tok::VecPushBack
        | Tok::VecPopBack
        | Tok::VecUnpack(_)
        | Tok::VecSwap
        | Tok::Freeze
        | Tok::ToU8
        | Tok::ToU16
        | Tok::ToU32
        | Tok::ToU64
        | Tok::ToU128
        | Tok::ToU256 => {
            let f = parse_builtin(tokens)?;
            FunctionCall_::Builtin(f)
        }
        Tok::DotNameValue => {
            let module_dot_name = parse_dot_name(tokens)?;
            let type_actuals = parse_type_actuals(tokens)?;
            let v: Vec<&str> = module_dot_name.split('.').collect();
            assert!(v.len() == 2);
            FunctionCall_::ModuleFunctionCall {
                module: ModuleName(Symbol::from(v[0])),
                name: FunctionName(Symbol::from(v[1])),
                type_actuals,
            }
        }
        t => {
            return Err(ParseError::InvalidToken {
                location: current_token_loc(tokens),
                message: format!(
                    "unrecognized token kind for qualified function name {:?}",
                    t
                ),
            });
        }
    };
    let end_loc = tokens.previous_end_loc();
    Ok(spanned(tokens.file_hash(), start_loc, end_loc, call))
}

// UnaryExp : Exp = {
//     "!" <e: Sp<UnaryExp>> => Exp::UnaryExp(UnaryOp::Not, Box::new(e)),
//     "*" <e: Sp<UnaryExp>> => Exp::Dereference(Box::new(e)),
//     "&mut " <e: Sp<UnaryExp>> "." <f: Field> => { ... },
//     "&" <e: Sp<UnaryExp>> "." <f: Field> => { ... },
//     CallOrTerm,
// }

fn parse_borrow_field_(
    tokens: &mut Lexer,
    mutable: bool,
) -> Result<Exp_, ParseError<Loc, anyhow::Error>> {
    // This could be either a field borrow (from UnaryExp) or
    // a borrow of a local variable (from Term). In the latter case,
    // only a simple name token is allowed, and it must not be
    // the start of a pack expression.
    let e = if tokens.peek() == Tok::NameValue {
        if tokens.lookahead()? != Tok::LBrace {
            let var = parse_var(tokens)?;
            return Ok(Exp_::BorrowLocal(mutable, var));
        }
        let start_loc = tokens.start_loc();
        let name = parse_name(tokens)?;
        let end_loc = tokens.previous_end_loc();
        let type_actuals: Vec<Type> = vec![];
        spanned(
            tokens.file_hash(),
            start_loc,
            end_loc,
            parse_pack_(tokens, name, type_actuals)?,
        )
    } else {
        parse_unary_exp(tokens)?
    };
    consume_token(tokens, Tok::Period)?;
    let field = parse_field_ident(tokens)?;
    Ok(Exp_::Borrow {
        is_mutable: mutable,
        exp: Box::new(e),
        field,
    })
}

fn parse_unary_exp_(tokens: &mut Lexer) -> Result<Exp_, ParseError<Loc, anyhow::Error>> {
    match tokens.peek() {
        Tok::Exclaim => {
            tokens.advance()?;
            let e = parse_unary_exp(tokens)?;
            Ok(Exp_::UnaryExp(UnaryOp::Not, Box::new(e)))
        }
        Tok::Star => {
            tokens.advance()?;
            let e = parse_unary_exp(tokens)?;
            Ok(Exp_::Dereference(Box::new(e)))
        }
        Tok::AmpMut => {
            tokens.advance()?;
            parse_borrow_field_(tokens, true)
        }
        Tok::Amp => {
            tokens.advance()?;
            parse_borrow_field_(tokens, false)
        }
        _ => parse_call_or_term_(tokens),
    }
}

fn parse_unary_exp(tokens: &mut Lexer) -> Result<Exp, ParseError<Loc, anyhow::Error>> {
    let start_loc = tokens.start_loc();
    let e = parse_unary_exp_(tokens)?;
    let end_loc = tokens.previous_end_loc();
    Ok(spanned(tokens.file_hash(), start_loc, end_loc, e))
}

// Call: Exp = {
//     <f: Sp<QualifiedFunctionName>> <exp: Sp<CallOrTerm>> => Exp::FunctionCall(f, Box::new(exp)),
// }

fn parse_call(
    tokens: &mut Lexer,
    f: FunctionCall,
    start_loc: usize,
) -> Result<Exp, ParseError<Loc, anyhow::Error>> {
    let exp = parse_call_or_term(tokens)?;
    let end_loc = tokens.previous_end_loc();
    Ok(spanned(
        tokens.file_hash(),
        start_loc,
        end_loc,
        Exp_::FunctionCall(f, Box::new(exp)),
    ))
}

// CallOrTerm: Exp = {
//     <f: Sp<QualifiedFunctionName>> <exp: Sp<CallOrTerm>> => Exp::FunctionCall(f, Box::new(exp)),
//     Term,
// }

fn parse_call_or_term_(tokens: &mut Lexer) -> Result<Exp_, ParseError<Loc, anyhow::Error>> {
    match tokens.peek() {
        Tok::VecPack(_)
        | Tok::VecLen
        | Tok::VecImmBorrow
        | Tok::VecMutBorrow
        | Tok::VecPushBack
        | Tok::VecPopBack
        | Tok::VecUnpack(_)
        | Tok::VecSwap
        | Tok::Freeze
        | Tok::ToU8
        | Tok::ToU16
        | Tok::ToU32
        | Tok::ToU64
        | Tok::ToU128
        | Tok::ToU256 => {
            let f = parse_qualified_function_name(tokens)?;
            let exp = parse_call_or_term(tokens)?;
            Ok(Exp_::FunctionCall(f, Box::new(exp)))
        }
        Tok::DotNameValue => {
            let f = parse_qualified_function_name(tokens)?;
            if tokens.peek() == Tok::LBrace {
                let FunctionCall_::ModuleFunctionCall {
                    module: ModuleName(enum_name),
                    name: FunctionName(variant_name),
                    type_actuals,
                } = f.value
                else {
                    return Err(ParseError::InvalidToken {
                        location: f.loc,
                        message: "Invalid variant pack call".to_string(),
                    });
                };
                parse_variant_pack_(tokens, enum_name, variant_name, type_actuals)
            } else {
                let exp = parse_call_or_term(tokens)?;
                Ok(Exp_::FunctionCall(f, Box::new(exp)))
            }
        }
        _ => parse_term_(tokens),
    }
}

fn parse_call_or_term(tokens: &mut Lexer) -> Result<Exp, ParseError<Loc, anyhow::Error>> {
    let start_loc = tokens.start_loc();
    let v = parse_call_or_term_(tokens)?;
    let end_loc = tokens.previous_end_loc();
    Ok(spanned(tokens.file_hash(), start_loc, end_loc, v))
}

// FieldExp: (Field_, Exp_) = {
//     <f: Sp<Field>> ":" <e: Sp<Exp>> => (f, e)
// }

fn parse_field_exp(tokens: &mut Lexer) -> Result<(Field, Exp), ParseError<Loc, anyhow::Error>> {
    let f = parse_field(tokens)?;
    consume_token(tokens, Tok::Colon)?;
    let e = parse_exp(tokens)?;
    Ok((f, e))
}

// Term: Exp = {
//     "move(" <v: Sp<Var>> ")" => Exp::Move(v),
//     "copy(" <v: Sp<Var>> ")" => Exp::Copy(v),
//     "&mut " <v: Sp<Var>> => Exp::BorrowLocal(true, v),
//     "&" <v: Sp<Var>> => Exp::BorrowLocal(false, v),
//     Sp<CopyableVal> => Exp::Value(<>),
//     <name_and_type_actuals: NameAndTypeActuals> "{" <fs:Comma<FieldExp>> "}" =>? { ... },
//     "(" <exps: Comma<Sp<Exp>>> ")" => Exp::ExprList(exps),
// }

fn parse_pack_(
    tokens: &mut Lexer,
    name: Symbol,
    type_actuals: Vec<Type>,
) -> Result<Exp_, ParseError<Loc, anyhow::Error>> {
    consume_token(tokens, Tok::LBrace)?;
    let fs = parse_comma_list(tokens, &[Tok::RBrace], parse_field_exp, true)?;
    consume_token(tokens, Tok::RBrace)?;
    Ok(Exp_::Pack(
        DatatypeName(name),
        type_actuals,
        fs.into_iter().collect::<Vec<_>>(),
    ))
}

fn parse_variant_pack_(
    tokens: &mut Lexer,
    enum_name: Symbol,
    variant_name: Symbol,
    type_actuals: Vec<Type>,
) -> Result<Exp_, ParseError<Loc, anyhow::Error>> {
    consume_token(tokens, Tok::LBrace)?;
    let fs = parse_comma_list(tokens, &[Tok::RBrace], parse_field_exp, true)?;
    consume_token(tokens, Tok::RBrace)?;
    Ok(Exp_::PackVariant(
        DatatypeName(enum_name),
        VariantName(variant_name),
        type_actuals,
        fs.into_iter().collect::<Vec<_>>(),
    ))
}

fn parse_term_(tokens: &mut Lexer) -> Result<Exp_, ParseError<Loc, anyhow::Error>> {
    match tokens.peek() {
        Tok::Move => {
            tokens.advance()?;
            let v = parse_var(tokens)?;
            consume_token(tokens, Tok::RParen)?;
            Ok(Exp_::Move(v))
        }
        Tok::Copy => {
            tokens.advance()?;
            let v = parse_var(tokens)?;
            consume_token(tokens, Tok::RParen)?;
            Ok(Exp_::Copy(v))
        }
        Tok::AmpMut => {
            tokens.advance()?;
            let v = parse_var(tokens)?;
            Ok(Exp_::BorrowLocal(true, v))
        }
        Tok::Amp => {
            tokens.advance()?;
            let v = parse_var(tokens)?;
            Ok(Exp_::BorrowLocal(false, v))
        }
        Tok::AccountAddressValue
        | Tok::True
        | Tok::False
        | Tok::U8Value
        | Tok::U16Value
        | Tok::U32Value
        | Tok::U64Value
        | Tok::U128Value
        | Tok::U256Value
        | Tok::ByteArrayValue => Ok(Exp_::Value(parse_copyable_val(tokens)?)),
        Tok::NameValue | Tok::NameBeginTyValue => {
            let (name, type_actuals) = parse_name_and_type_actuals(tokens)?;
            parse_pack_(tokens, name, type_actuals)
        }
        Tok::LParen => {
            tokens.advance()?;
            let exps = parse_comma_list(tokens, &[Tok::RParen], parse_exp, true)?;
            consume_token(tokens, Tok::RParen)?;
            Ok(Exp_::ExprList(exps))
        }
        Tok::At => {
            tokens.advance()?;
            let address = parse_account_address(tokens)?;
            Ok(Exp_::address(address).value)
        }
        t => Err(ParseError::InvalidToken {
            location: current_token_loc(tokens),
            message: format!("unrecognized token kind for term {:?}", t),
        }),
    }
}

// QualifiedStructIdent : QualifiedStructIdent = {
//     <module_dot_struct: DotName> =>? { ... }
// }

fn parse_qualified_struct_ident(
    tokens: &mut Lexer,
) -> Result<QualifiedDatatypeIdent, ParseError<Loc, anyhow::Error>> {
    let module_dot_struct = parse_dot_name(tokens)?;
    let v: Vec<&str> = module_dot_struct.split('.').collect();
    assert!(v.len() == 2);
    let m: ModuleName = ModuleName(Symbol::from(v[0]));
    let n: DatatypeName = DatatypeName(Symbol::from(v[1]));
    Ok(QualifiedDatatypeIdent::new(m, n))
}

// ModuleName: ModuleName = {
//     <n: Name> =>? ModuleName::parse(n),
// }

fn parse_module_name(tokens: &mut Lexer) -> Result<ModuleName, ParseError<Loc, anyhow::Error>> {
    Ok(ModuleName(parse_name(tokens)?))
}

// Builtin: Builtin = {
//     "exists<" <name_and_type_actuals: NameAndTypeActuals> ">" =>? { ... },
//     "borrow_global<" <name_and_type_actuals: NameAndTypeActuals> ">" =>? { ... },
//     "borrow_global_mut<" <name_and_type_actuals: NameAndTypeActuals> ">" =>? { ... },
//     "move_to<" <name_and_type_actuals: NameAndTypeActuals> ">" =>? { ... },
//     "move_from<" <name_and_type_actuals: NameAndTypeActuals> ">" =>? { ... },
//     "vec_*<" <type_actuals: TypeActuals> ">" =>? { ... },
//     "freeze" => Builtin::Freeze,
// }

fn parse_builtin(tokens: &mut Lexer) -> Result<Builtin, ParseError<Loc, anyhow::Error>> {
    match tokens.peek() {
        Tok::VecPack(num) => {
            tokens.advance()?;
            let type_actuals = parse_type_actuals(tokens)?;
            Ok(Builtin::VecPack(type_actuals, num))
        }
        Tok::VecLen => {
            tokens.advance()?;
            let type_actuals = parse_type_actuals(tokens)?;
            Ok(Builtin::VecLen(type_actuals))
        }
        Tok::VecImmBorrow => {
            tokens.advance()?;
            let type_actuals = parse_type_actuals(tokens)?;
            Ok(Builtin::VecImmBorrow(type_actuals))
        }
        Tok::VecMutBorrow => {
            tokens.advance()?;
            let type_actuals = parse_type_actuals(tokens)?;
            Ok(Builtin::VecMutBorrow(type_actuals))
        }
        Tok::VecPushBack => {
            tokens.advance()?;
            let type_actuals = parse_type_actuals(tokens)?;
            Ok(Builtin::VecPushBack(type_actuals))
        }
        Tok::VecPopBack => {
            tokens.advance()?;
            let type_actuals = parse_type_actuals(tokens)?;
            Ok(Builtin::VecPopBack(type_actuals))
        }
        Tok::VecUnpack(num) => {
            tokens.advance()?;
            let type_actuals = parse_type_actuals(tokens)?;
            Ok(Builtin::VecUnpack(type_actuals, num))
        }
        Tok::VecSwap => {
            tokens.advance()?;
            let type_actuals = parse_type_actuals(tokens)?;
            Ok(Builtin::VecSwap(type_actuals))
        }
        Tok::Freeze => {
            tokens.advance()?;
            Ok(Builtin::Freeze)
        }
        Tok::ToU8 => {
            tokens.advance()?;
            Ok(Builtin::ToU8)
        }
        Tok::ToU16 => {
            tokens.advance()?;
            Ok(Builtin::ToU16)
        }
        Tok::ToU32 => {
            tokens.advance()?;
            Ok(Builtin::ToU32)
        }
        Tok::ToU64 => {
            tokens.advance()?;
            Ok(Builtin::ToU64)
        }
        Tok::ToU128 => {
            tokens.advance()?;
            Ok(Builtin::ToU128)
        }
        Tok::ToU256 => {
            tokens.advance()?;
            Ok(Builtin::ToU256)
        }
        t => Err(ParseError::InvalidToken {
            location: current_token_loc(tokens),
            message: format!("unrecognized token kind for builtin {:?}", t),
        }),
    }
}

// LValue: LValue = {
//     <l:Sp<Var>> => LValue::Var(l),
//     "*" <e: Sp<Exp>> => LValue::Mutate(e),
//     "_" => LValue::Pop,
// }

fn parse_lvalue_(tokens: &mut Lexer) -> Result<LValue_, ParseError<Loc, anyhow::Error>> {
    match tokens.peek() {
        Tok::NameValue => {
            let l = parse_var(tokens)?;
            Ok(LValue_::Var(l))
        }
        Tok::Star => {
            tokens.advance()?;
            let e = parse_exp(tokens)?;
            Ok(LValue_::Mutate(e))
        }
        Tok::Underscore => {
            tokens.advance()?;
            Ok(LValue_::Pop)
        }
        t => Err(ParseError::InvalidToken {
            location: current_token_loc(tokens),
            message: format!("unrecognized token kind for lvalue {:?}", t),
        }),
    }
}

fn parse_lvalue(tokens: &mut Lexer) -> Result<LValue, ParseError<Loc, anyhow::Error>> {
    let start_loc = tokens.start_loc();
    let lv = parse_lvalue_(tokens)?;
    let end_loc = tokens.previous_end_loc();
    Ok(spanned(tokens.file_hash(), start_loc, end_loc, lv))
}

// FieldBindings: (Field_, Var_) = {
//     <f: Sp<Field>> ":" <v: Sp<Var>> => (f, v),
//     <f: Sp<Field>> => { ... }
// }

fn parse_field_bindings(
    tokens: &mut Lexer,
) -> Result<(Field, Var), ParseError<Loc, anyhow::Error>> {
    let f = parse_field(tokens)?;
    if tokens.peek() == Tok::Colon {
        tokens.advance()?; // consume the colon
        let v = parse_var(tokens)?;
        Ok((f, v))
    } else {
        Ok((
            f.clone(),
            Spanned {
                loc: f.loc,
                value: Var_(f.value.0),
            },
        ))
    }
}

// pub Cmd : Cmd = {
//     <lvalues: Comma<Sp<LValue>>> "=" <e: Sp<Exp>> => Cmd::Assign(lvalues, e),
//     <name_and_type_actuals: NameAndTypeActuals> "{" <bindings: Comma<FieldBindings>> "}" "=" <e: Sp<Exp>> =>? { ... },
//     "abort" <err: Sp<Exp>?> => { ... },
//     "return" <v: Comma<Sp<Exp>>> => Cmd::Return(Box::new(Spanned::unsafe_no_loc(Exp::ExprList(v)))),
//     "continue" => Cmd::Continue,
//     "break" => Cmd::Break,
//     <Sp<Call>> => Cmd::Exp(Box::new(<>)),
//     "(" <Comma<Sp<Exp>>> ")" => Cmd::Exp(Box::new(Spanned::unsafe_no_loc(Exp::ExprList(<>)))),
// }

fn parse_assign_(tokens: &mut Lexer) -> Result<Statement_, ParseError<Loc, anyhow::Error>> {
    let lvalues = parse_comma_list(tokens, &[Tok::Equal], parse_lvalue, false)?;
    if lvalues.is_empty() {
        return Err(ParseError::InvalidToken {
            location: current_token_loc(tokens),
            message: "could not parse lvalues in assignment".to_string(),
        });
    }
    consume_token(tokens, Tok::Equal)?;
    let e = parse_exp(tokens)?;
    Ok(Statement_::Assign(lvalues, e))
}

fn parse_unpack_(
    tokens: &mut Lexer,
    name: Symbol,
    type_actuals: Vec<Type>,
) -> Result<Statement_, ParseError<Loc, anyhow::Error>> {
    consume_token(tokens, Tok::LBrace)?;
    let bindings = parse_comma_list(tokens, &[Tok::RBrace], parse_field_bindings, true)?;
    consume_token(tokens, Tok::RBrace)?;
    consume_token(tokens, Tok::Equal)?;
    let e = parse_exp(tokens)?;
    Ok(Statement_::Unpack(
        DatatypeName(name),
        type_actuals,
        bindings.into_iter().collect(),
        Box::new(e),
    ))
}

// <enum>.<variant>("<" <type_actuals> ">")? { <bindings> } (&|&mut)? = <exp>
fn parse_variant_unpack_(
    tokens: &mut Lexer,
    enum_name: Symbol,
    variant_name: Symbol,
    type_actuals: Vec<Type>,
    unpack_type: UnpackType,
) -> Result<Statement_, ParseError<Loc, anyhow::Error>> {
    consume_token(tokens, Tok::LBrace)?;
    let bindings = parse_comma_list(tokens, &[Tok::RBrace], parse_field_bindings, true)?;
    consume_token(tokens, Tok::RBrace)?;
    consume_token(tokens, Tok::Equal)?;
    let e = parse_exp(tokens)?;
    Ok(Statement_::UnpackVariant(
        DatatypeName(enum_name),
        VariantName(variant_name),
        type_actuals,
        bindings.into_iter().collect(),
        Box::new(e),
        unpack_type,
    ))
}

// variant_switch <enum_name> e { (<variant_name> => <lbl>)* }
fn parse_variant_switch_(tokens: &mut Lexer) -> Result<Statement_, ParseError<Loc, anyhow::Error>> {
    let name = parse_name(tokens)?;
    let e = parse_exp(tokens)?;
    consume_token(tokens, Tok::LBrace)?;
    let lbls = parse_comma_list(tokens, &[Tok::RBrace], parse_variant_switch_arm, true)?;
    consume_token(tokens, Tok::RBrace)?;
    Ok(Statement_::VariantSwitch(
        DatatypeName(name),
        lbls,
        Box::new(e),
    ))
}

// <variant_name> : <lbl>
fn parse_variant_switch_arm(
    tokens: &mut Lexer,
) -> Result<(VariantName, BlockLabel), ParseError<Loc, anyhow::Error>> {
    let v = parse_name(tokens)?;
    consume_token(tokens, Tok::Colon)?;
    let lbl = parse_label(tokens)?;
    Ok((VariantName(v), lbl))
}

/// Parses a statement.
fn parse_statement_(tokens: &mut Lexer) -> Result<Statement_, ParseError<Loc, anyhow::Error>> {
    match tokens.peek() {
        Tok::Abort => {
            tokens.advance()?;
            let val = if tokens.peek() == Tok::Semicolon {
                None
            } else {
                Some(Box::new(parse_exp(tokens)?))
            };
            Ok(Statement_::Abort(val))
        }
        Tok::Assert => {
            tokens.advance()?;
            let e = parse_exp(tokens)?;
            consume_token(tokens, Tok::Comma)?;
            let err = parse_exp(tokens)?;
            consume_token(tokens, Tok::RParen)?;
            let cond = {
                let loc = e.loc;
                sp(loc, Exp_::UnaryExp(UnaryOp::Not, Box::new(e)))
            };
            Ok(Statement_::Assert(Box::new(cond), Box::new(err)))
        }
        Tok::Jump => {
            consume_token(tokens, Tok::Jump)?;
            Ok(Statement_::Jump(parse_label(tokens)?))
        }
        Tok::JumpIf => {
            consume_token(tokens, Tok::JumpIf)?;
            consume_token(tokens, Tok::LParen)?;
            let cond = parse_exp(tokens)?;
            consume_token(tokens, Tok::RParen)?;
            Ok(Statement_::JumpIf(Box::new(cond), parse_label(tokens)?))
        }
        Tok::JumpIfFalse => {
            consume_token(tokens, Tok::JumpIfFalse)?;
            consume_token(tokens, Tok::LParen)?;
            let cond = parse_exp(tokens)?;
            consume_token(tokens, Tok::RParen)?;
            Ok(Statement_::JumpIfFalse(
                Box::new(cond),
                parse_label(tokens)?,
            ))
        }
        Tok::NameValue => {
            // This could be either an LValue for an assignment or
            // NameAndTypeActuals (with no type_actuals) for an unpack.
            if tokens.lookahead()? == Tok::LBrace {
                let name = parse_name(tokens)?;
                parse_unpack_(tokens, name, vec![])
            } else {
                parse_assign_(tokens)
            }
        }
        Tok::Return => {
            tokens.advance()?;
            let start = tokens.start_loc();
            let v = parse_comma_list(tokens, &[Tok::Semicolon], parse_exp, true)?;
            let end = tokens.start_loc();
            Ok(Statement_::Return(Box::new(spanned(
                tokens.file_hash(),
                start,
                end,
                Exp_::ExprList(v),
            ))))
        }
        Tok::Star | Tok::Underscore => parse_assign_(tokens),
        Tok::NameBeginTyValue => {
            let (name, tys) = parse_name_and_type_actuals(tokens)?;
            parse_unpack_(tokens, name, tys)
        }
        Tok::VariantSwitch => {
            consume_token(tokens, Tok::VariantSwitch)?;
            parse_variant_switch_(tokens)
        }
        Tok::VecPack(_)
        | Tok::VecLen
        | Tok::VecImmBorrow
        | Tok::VecMutBorrow
        | Tok::VecPushBack
        | Tok::VecPopBack
        | Tok::VecUnpack(_)
        | Tok::VecSwap
        | Tok::Freeze
        | Tok::ToU8
        | Tok::ToU16
        | Tok::ToU32
        | Tok::ToU64
        | Tok::ToU128
        | Tok::DotNameValue
        | Tok::ToU256 => {
            let start_loc = tokens.start_loc();
            let f = parse_qualified_function_name(tokens)?;
            if tokens.peek() == Tok::LBrace {
                let FunctionCall_::ModuleFunctionCall {
                    module: ModuleName(enum_name),
                    name: FunctionName(variant_name),
                    type_actuals,
                } = f.value
                else {
                    return Err(ParseError::InvalidToken {
                        location: f.loc,
                        message: "Invalid variant unpack call".to_string(),
                    });
                };
                parse_variant_unpack_(
                    tokens,
                    enum_name,
                    variant_name,
                    type_actuals,
                    UnpackType::ByValue,
                )
            } else {
                Ok(Statement_::Exp(Box::new(parse_call(tokens, f, start_loc)?)))
            }
        }
        x @ (Tok::Amp | Tok::AmpMut) => {
            let start_loc = current_token_loc(tokens);
            tokens.advance()?;
            let f = parse_qualified_function_name(tokens)?;
            if tokens.peek() == Tok::LBrace {
                let FunctionCall_::ModuleFunctionCall {
                    module: ModuleName(enum_name),
                    name: FunctionName(variant_name),
                    type_actuals,
                } = f.value
                else {
                    return Err(ParseError::InvalidToken {
                        location: f.loc,
                        message: "Invalid variant unpack call".to_string(),
                    });
                };
                let unpack_type = match x {
                    Tok::Amp => UnpackType::ByImmRef,
                    Tok::AmpMut => UnpackType::ByMutRef,
                    _ => unreachable!(),
                };
                parse_variant_unpack_(tokens, enum_name, variant_name, type_actuals, unpack_type)
            } else {
                Err(ParseError::InvalidToken {
                    location: start_loc,
                    message: format!("invalid token kind for statement {:?}", x),
                })
            }
        }
        Tok::LParen => {
            tokens.advance()?;
            let start = tokens.start_loc();
            let v = parse_comma_list(tokens, &[Tok::RParen], parse_exp, true)?;
            consume_token(tokens, Tok::RParen)?;
            let end = tokens.start_loc();
            Ok(Statement_::Exp(Box::new(spanned(
                tokens.file_hash(),
                start,
                end,
                Exp_::ExprList(v),
            ))))
        }
        t => Err(ParseError::InvalidToken {
            location: current_token_loc(tokens),
            message: format!("invalid token kind for statement {:?}", t),
        }),
    }
}

/// Parses a statement with its location.
fn parse_statement(tokens: &mut Lexer) -> Result<Statement, ParseError<Loc, anyhow::Error>> {
    let start_loc = tokens.start_loc();
    let c = parse_statement_(tokens)?;
    let end_loc = tokens.previous_end_loc();
    let cmd = spanned(tokens.file_hash(), start_loc, end_loc, c);
    consume_token(tokens, Tok::Semicolon)?;
    Ok(cmd)
}

/// Parses a label declaration for a block, e.g.: `label b0:`.
fn parse_block_label(tokens: &mut Lexer) -> Result<BlockLabel, ParseError<Loc, anyhow::Error>> {
    consume_token(tokens, Tok::Label)?;
    let label = parse_label(tokens)?;
    consume_token(tokens, Tok::Colon)?;
    Ok(label)
}

/// Parses a label identifier, e.g.: the `b0` in the statement `jump b0;`.
fn parse_label(tokens: &mut Lexer) -> Result<BlockLabel, ParseError<Loc, anyhow::Error>> {
    let start = tokens.start_loc();
    let name = parse_name(tokens)?;
    let end = tokens.previous_end_loc();
    Ok(spanned(tokens.file_hash(), start, end, BlockLabel_(name)))
}

/// Parses a sequence of blocks, such as would appear within the `{` and `}` delimiters of a
/// function body.
fn parse_blocks(tokens: &mut Lexer) -> Result<Vec<Block>, ParseError<Loc, anyhow::Error>> {
    let mut blocks = vec![];
    while tokens.peek() != Tok::RBrace {
        blocks.push(parse_block(tokens)?);
    }
    Ok(blocks)
}

/// Parses a block: its block label `label b:`, and a sequence of 0 or more statements.
fn parse_block(tokens: &mut Lexer) -> Result<Block, ParseError<Loc, anyhow::Error>> {
    let start_loc = tokens.start_loc();
    let label = parse_block_label(tokens)?;
    let mut statements = vec![];
    while !matches!(tokens.peek(), Tok::Label | Tok::RBrace) {
        statements.push(parse_statement(tokens)?);
    }
    Ok(spanned(
        tokens.file_hash(),
        start_loc,
        tokens.previous_end_loc(),
        Block_::new(label, statements),
    ))
}

// Declaration: (Var_, Type) = {
//   "let" <v: Sp<Var>> ":" <t: Type> ";" => (v, t),
// }

fn parse_declaration(tokens: &mut Lexer) -> Result<(Var, Type), ParseError<Loc, anyhow::Error>> {
    consume_token(tokens, Tok::Let)?;
    let v = parse_var(tokens)?;
    consume_token(tokens, Tok::Colon)?;
    let t = parse_type(tokens)?;
    consume_token(tokens, Tok::Semicolon)?;
    Ok((v, t))
}

// Declarations: Vec<(Var_, Type)> = {
//     <Declaration*>
// }

fn parse_declarations(
    tokens: &mut Lexer,
) -> Result<Vec<(Var, Type)>, ParseError<Loc, anyhow::Error>> {
    let mut decls: Vec<(Var, Type)> = vec![];
    // Declarations always begin with the "let" token so continue parsing
    // them until we hit something else.
    while tokens.peek() == Tok::Let {
        decls.push(parse_declaration(tokens)?);
    }
    Ok(decls)
}

// FunctionBlock: (Vec<(Var_, Type)>, Block) = {
//     "{" <locals: Declarations> <stmts: Statements> "}" => (locals, Block::new(stmts))
// }

fn parse_function_block_(
    tokens: &mut Lexer,
) -> Result<(Vec<(Var, Type)>, Vec<Block>), ParseError<Loc, anyhow::Error>> {
    consume_token(tokens, Tok::LBrace)?;
    let locals = parse_declarations(tokens)?;
    let statements = parse_blocks(tokens)?;
    consume_token(tokens, Tok::RBrace)?;
    Ok((locals, statements))
}

fn token_to_ability(token: Tok, contents: &str) -> Option<Ability> {
    match (token, contents) {
        (Tok::Copy, _) => Some(Ability::Copy),
        (Tok::NameValue, Ability::DROP) => Some(Ability::Drop),
        (Tok::NameValue, Ability::STORE) => Some(Ability::Store),
        (Tok::NameValue, Ability::KEY) => Some(Ability::Key),
        _ => None,
    }
}

// Ability: Ability = {
//     "copy" => Ability::Copy,
//     "drop" => Ability::Drop,
//     "store" => Ability::Store,
//     "key" => Ability::Key,
// }
fn parse_ability(tokens: &mut Lexer) -> Result<(Ability, Loc), ParseError<Loc, anyhow::Error>> {
    let a = match token_to_ability(tokens.peek(), tokens.content()) {
        Some(a) => (a, current_token_loc(tokens)),
        None => {
            return Err(ParseError::InvalidToken {
                location: current_token_loc(tokens),
                message: "could not parse ability".to_string(),
            });
        }
    };
    tokens.advance()?;
    Ok(a)
}

// Type: Type = {
//     "address" => Type::Address,
//     "signer" => Type::Signer,
//     "u64" => Type::U64,
//     "bool" => Type::Bool,
//     "bytearray" => Type::ByteArray,
//     <s: QualifiedStructIdent> <tys: TypeActuals> => Type::Struct(s, tys),
//     "&" <t: Type> => Type::Reference(false, Box::new(t)),
//     "&mut " <t: Type> => Type::Reference(true, Box::new(t)),
//     <n: Name> =>? Ok(Type::TypeParameter(TypeVar::parse(n)?)),
// }

fn parse_type(tokens: &mut Lexer) -> Result<Type, ParseError<Loc, anyhow::Error>> {
    let start_loc = tokens.start_loc();
    let t = match tokens.peek() {
        Tok::NameValue if matches!(tokens.content(), "address") => {
            tokens.advance()?;
            Type_::Address
        }
        Tok::NameValue if matches!(tokens.content(), "u8") => {
            tokens.advance()?;
            Type_::U8
        }
        Tok::NameValue if matches!(tokens.content(), "u16") => {
            tokens.advance()?;
            Type_::U16
        }
        Tok::NameValue if matches!(tokens.content(), "u32") => {
            tokens.advance()?;
            Type_::U32
        }
        Tok::NameValue if matches!(tokens.content(), "u64") => {
            tokens.advance()?;
            Type_::U64
        }
        Tok::NameValue if matches!(tokens.content(), "u128") => {
            tokens.advance()?;
            Type_::U128
        }
        Tok::NameValue if matches!(tokens.content(), "u256") => {
            tokens.advance()?;
            Type_::U256
        }
        Tok::NameValue if matches!(tokens.content(), "bool") => {
            tokens.advance()?;
            Type_::Bool
        }
        Tok::NameValue if matches!(tokens.content(), "signer") => {
            tokens.advance()?;
            Type_::Signer
        }
        Tok::NameBeginTyValue if matches!(tokens.content(), "vector<") => {
            tokens.advance()?;
            let ty = parse_type(tokens)?;
            adjust_token(tokens, &[Tok::Greater])?;
            consume_token(tokens, Tok::Greater)?;
            Type_::Vector(Box::new(ty))
        }
        Tok::DotNameValue => {
            let s = parse_qualified_struct_ident(tokens)?;
            let tys = parse_type_actuals(tokens)?;
            Type_::Datatype(s, tys)
        }
        Tok::Amp => {
            tokens.advance()?;
            Type_::Reference(false, Box::new(parse_type(tokens)?))
        }
        Tok::AmpMut => {
            tokens.advance()?;
            Type_::Reference(true, Box::new(parse_type(tokens)?))
        }
        Tok::NameValue => Type_::TypeParameter(TypeVar_(parse_name(tokens)?)),
        t => {
            return Err(ParseError::InvalidToken {
                location: current_token_loc(tokens),
                message: format!("invalid token kind for type {:?}", t),
            });
        }
    };
    let end_loc = tokens.previous_end_loc();
    Ok(spanned(tokens.file_hash(), start_loc, end_loc, t))
}

// TypeVar: TypeVar = {
//     <n: Name> =>? TypeVar::parse(n),
// }
// TypeVar_ = Sp<TypeVar>;

fn parse_type_var(tokens: &mut Lexer) -> Result<TypeVar, ParseError<Loc, anyhow::Error>> {
    let start_loc = tokens.start_loc();
    let type_var = TypeVar_(parse_name(tokens)?);
    let end_loc = tokens.previous_end_loc();
    Ok(spanned(tokens.file_hash(), start_loc, end_loc, type_var))
}

fn parse_type_parameter_with_phantom_decl(
    tokens: &mut Lexer,
) -> Result<DatatypeTypeParameter, ParseError<Loc, anyhow::Error>> {
    let is_phantom = if tokens.peek() == Tok::NameValue && tokens.content() == "phantom" {
        tokens.advance()?;
        true
    } else {
        false
    };
    let (type_var, abilities) = parse_type_parameter(tokens)?;
    Ok((is_phantom, type_var, abilities))
}

// TypeFormal: (TypeVar_, Kind) = {
//     <type_var: Sp<TypeVar>> <k: (":" <Ability> ("+" <Ability>)*)?> =>? {
// }

fn parse_type_parameter(
    tokens: &mut Lexer,
) -> Result<(TypeVar, BTreeSet<Ability>), ParseError<Loc, anyhow::Error>> {
    let type_var = parse_type_var(tokens)?;
    if tokens.peek() == Tok::Colon {
        tokens.advance()?; // consume the ":"
        let abilities = parse_list(
            tokens,
            |tokens| {
                if tokens.peek() == Tok::Plus {
                    tokens.advance()?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            },
            parse_ability,
        )?;
        let mut ability_set = BTreeSet::new();
        for (ability, location) in abilities {
            let was_new_element = ability_set.insert(ability);
            if !was_new_element {
                return Err(ParseError::User {
                    location,
                    error: anyhow!("Duplicate ability '{}'", ability),
                });
            }
        }
        Ok((type_var, ability_set))
    } else {
        Ok((type_var, BTreeSet::new()))
    }
}

// TypeActuals: Vec<Type> = {
//     <tys: ('<' <Comma<Type>> ">")?> => { ... }
// }

fn parse_type_actuals(tokens: &mut Lexer) -> Result<Vec<Type>, ParseError<Loc, anyhow::Error>> {
    let tys = if tokens.peek() == Tok::Less {
        tokens.advance()?; // consume the '<'
        let list = parse_comma_list(tokens, &[Tok::Greater], parse_type, true)?;
        consume_token(tokens, Tok::Greater)?;
        list
    } else {
        vec![]
    };
    Ok(tys)
}

// NameAndTypeFormals: (String, Vec<(TypeVar_, Kind)>) = {
//     <n: NameBeginTy> <k: Comma<TypeFormal>> ">" => (n, k),
//     <n: Name> => (n, vec![]),
// }

fn parse_name_and_type_parameters<T, F>(
    tokens: &mut Lexer,
    param_parser: F,
) -> Result<(Symbol, Vec<T>), ParseError<Loc, anyhow::Error>>
where
    F: Fn(&mut Lexer) -> Result<T, ParseError<Loc, anyhow::Error>>,
{
    let mut has_types = false;
    let n = if tokens.peek() == Tok::NameBeginTyValue {
        has_types = true;
        parse_name_begin_ty(tokens)?
    } else {
        parse_name(tokens)?
    };
    let k = if has_types {
        let list = parse_comma_list(tokens, &[Tok::Greater], param_parser, true)?;
        consume_token(tokens, Tok::Greater)?;
        list
    } else {
        vec![]
    };
    Ok((n, k))
}

// NameAndTypeActuals: (String, Vec<Type>) = {
//     <n: NameBeginTy> '<' <tys: Comma<Type>> ">" => (n, tys),
//     <n: Name> => (n, vec![]),
// }

fn parse_name_and_type_actuals(
    tokens: &mut Lexer,
) -> Result<(Symbol, Vec<Type>), ParseError<Loc, anyhow::Error>> {
    let mut has_types = false;
    let n = if tokens.peek() == Tok::NameBeginTyValue {
        has_types = true;
        parse_name_begin_ty(tokens)?
    } else {
        parse_name(tokens)?
    };
    let tys = if has_types {
        let list = parse_comma_list(tokens, &[Tok::Greater], parse_type, true)?;
        consume_token(tokens, Tok::Greater)?;
        list
    } else {
        vec![]
    };
    Ok((n, tys))
}

// ArgDecl : (Var_, Type) = {
//     <v: Sp<Var>> ":" <t: Type> => (v, t)
// }

fn parse_arg_decl(tokens: &mut Lexer) -> Result<(Var, Type), ParseError<Loc, anyhow::Error>> {
    let v = parse_var(tokens)?;
    consume_token(tokens, Tok::Colon)?;
    let t = parse_type(tokens)?;
    Ok((v, t))
}

// ReturnType: Vec<Type> = {
//     ":" <t: Type> <v: ("*" <Type>)*> => { ... }
// }

fn parse_return_type(tokens: &mut Lexer) -> Result<Vec<Type>, ParseError<Loc, anyhow::Error>> {
    consume_token(tokens, Tok::Colon)?;
    let t = parse_type(tokens)?;
    let mut v = vec![t];
    while tokens.peek() == Tok::Star {
        tokens.advance()?;
        v.push(parse_type(tokens)?);
    }
    Ok(v)
}

// FunctionVisibility : FunctionVisibility = {
//   (Public("("<v: Script | Friend>")")?)?
// }
fn parse_function_visibility(
    tokens: &mut Lexer,
) -> Result<FunctionVisibility, ParseError<Loc, anyhow::Error>> {
    let visibility = if match_token(tokens, Tok::Public)? {
        let sub_public_vis = if match_token(tokens, Tok::LParen)? {
            let sub_token = tokens.peek();
            match &sub_token {
                Tok::Script | Tok::Friend => (),
                t => {
                    return Err(ParseError::InvalidToken {
                        location: current_token_loc(tokens),
                        message: format!("expected Tok::Script or Tok::Friend, not {:?}", t),
                    });
                }
            }
            tokens.advance()?;
            consume_token(tokens, Tok::RParen)?;
            Some(sub_token)
        } else {
            None
        };
        match sub_public_vis {
            None => FunctionVisibility::Public,
            Some(Tok::Friend) => FunctionVisibility::Friend,
            _ => panic!("Unexpected token that is not a visibility modifier"),
        }
    } else {
        FunctionVisibility::Internal
    };
    Ok(visibility)
}

// FunctionDecl : (FunctionName, Function_) = {
//   <f: Sp<MoveFunctionDecl>> => (f.value.0, Spanned { span: f.loc, value: f.value.1 }),
//   <f: Sp<NativeFunctionDecl>> => (f.value.0, Spanned { span: f.loc, value: f.value.1 }),
// }

// MoveFunctionDecl : (FunctionName, Function) = {
//     <v: FunctionVisibility> <name_and_type_parameters: NameAndTypeFormals>
//     "(" <args: (ArgDecl)*> ")" <ret: ReturnType?>
//         <acquires: AcquireList?>
//         <locals_body: FunctionBlock> =>? { ... }
// }

// NativeFunctionDecl: (FunctionName, Function) = {
//     <nat: NativeTag> <v: FunctionVisibility> <name_and_type_parameters: NameAndTypeFormals>
//     "(" <args: Comma<ArgDecl>> ")" <ret: ReturnType?>
//         <acquires: AcquireList?>
//         ";" =>? { ... }
// }

fn parse_function_decl(
    tokens: &mut Lexer,
) -> Result<(FunctionName, Function), ParseError<Loc, anyhow::Error>> {
    let start_loc = tokens.start_loc();

    let is_native = if tokens.peek() == Tok::Native {
        tokens.advance()?;
        true
    } else {
        false
    };

    let visibility = parse_function_visibility(tokens)?;
    let is_entry = if tokens.peek() == Tok::NameValue && tokens.content() == "entry" {
        tokens.advance()?;
        true
    } else {
        false
    };

    let (name, type_parameters) = parse_name_and_type_parameters(tokens, parse_type_parameter)?;
    consume_token(tokens, Tok::LParen)?;
    let args = parse_comma_list(tokens, &[Tok::RParen], parse_arg_decl, true)?;
    consume_token(tokens, Tok::RParen)?;

    let ret = if tokens.peek() == Tok::Colon {
        Some(parse_return_type(tokens)?)
    } else {
        None
    };

    let body = if is_native {
        consume_token(tokens, Tok::Semicolon)?;
        FunctionBody::Native
    } else {
        let (locals, body) = parse_function_block_(tokens)?;
        FunctionBody::Move { locals, code: body }
    };

    let end_loc = tokens.previous_end_loc();
    let func_name = FunctionName(name);
    let func = Function_::new(
        make_loc(tokens.file_hash(), start_loc, end_loc),
        visibility,
        is_entry,
        args,
        ret.unwrap_or_default(),
        type_parameters,
        body,
    );

    Ok((
        func_name,
        spanned(tokens.file_hash(), start_loc, end_loc, func),
    ))
}

// FieldDecl : (Field_, Type) = {
//     <f: Sp<Field>> ":" <t: Type> => (f, t)
// }

fn parse_field_decl(tokens: &mut Lexer) -> Result<(Field, Type), ParseError<Loc, anyhow::Error>> {
    let f = parse_field(tokens)?;
    consume_token(tokens, Tok::Colon)?;
    let t = parse_type(tokens)?;
    Ok((f, t))
}

// StructDecl: StructDefinition_ = {
//     "struct" <name_and_type_parameters:
//     NameAndTypeFormals> ("has" <Ability> ("," <Ability)*)? "{" <data: Comma<FieldDecl>> "}"
//     =>? { ... }
//     <native: NativeTag> <name_and_type_parameters: NameAndTypeFormals>
//     ("has" <Ability> ("," <Ability)*)?";" =>? { ... }
// }
fn parse_struct_decl(
    tokens: &mut Lexer,
) -> Result<StructDefinition, ParseError<Loc, anyhow::Error>> {
    let start_loc = tokens.start_loc();

    let is_native = if tokens.peek() == Tok::Native {
        tokens.advance()?;
        true
    } else {
        false
    };

    consume_token(tokens, Tok::Struct)?;
    let (name, type_parameters) =
        parse_name_and_type_parameters(tokens, parse_type_parameter_with_phantom_decl)?;

    let mut abilities = BTreeSet::new();
    if tokens.peek() == Tok::NameValue && tokens.content() == "has" {
        tokens.advance()?;
        let abilities_vec =
            parse_comma_list(tokens, &[Tok::LBrace, Tok::Semicolon], parse_ability, false)?;
        for (ability, location) in abilities_vec {
            let was_new_element = abilities.insert(ability);
            if !was_new_element {
                return Err(ParseError::User {
                    location,
                    error: anyhow!("Duplicate ability '{}'", ability),
                });
            }
        }
    }

    if is_native {
        consume_token(tokens, Tok::Semicolon)?;
        let end_loc = tokens.previous_end_loc();
        return Ok(spanned(
            tokens.file_hash(),
            start_loc,
            end_loc,
            StructDefinition_::native(abilities, name, type_parameters),
        ));
    }

    consume_token(tokens, Tok::LBrace)?;
    let fields = parse_comma_list(tokens, &[Tok::RBrace], parse_field_decl, true)?;
    consume_token(tokens, Tok::RBrace)?;
    let end_loc = tokens.previous_end_loc();
    Ok(spanned(
        tokens.file_hash(),
        start_loc,
        end_loc,
        StructDefinition_::move_declared(abilities, name, type_parameters, fields),
    ))
}

// EnumDecl: EnumDefinition = {
//     "enum" <name_and_type_parameters:
//     NameAndTypeFormals> ("has" <Ability> ("," <Ability)*)? "{" <data: Comma<VariantDecl>> "}"
//     => { ... }
// }
fn parse_enum_decl(tokens: &mut Lexer) -> Result<EnumDefinition, ParseError<Loc, anyhow::Error>> {
    let start_loc = tokens.start_loc();

    consume_token(tokens, Tok::Enum)?;

    let (name, type_parameters) =
        parse_name_and_type_parameters(tokens, parse_type_parameter_with_phantom_decl)?;

    let mut abilities = BTreeSet::new();
    if tokens.peek() == Tok::NameValue && tokens.content() == "has" {
        tokens.advance()?;
        let abilities_vec =
            parse_comma_list(tokens, &[Tok::LBrace, Tok::Semicolon], parse_ability, false)?;
        for (ability, location) in abilities_vec {
            let was_new_element = abilities.insert(ability);
            if !was_new_element {
                return Err(ParseError::User {
                    location,
                    error: anyhow!("Duplicate ability '{}'", ability),
                });
            }
        }
    }

    consume_token(tokens, Tok::LBrace)?;
    let variants = parse_comma_list(tokens, &[Tok::RBrace], parse_variant_decl, true)?;
    consume_token(tokens, Tok::RBrace)?;
    let end_loc = tokens.previous_end_loc();
    Ok(spanned(
        tokens.file_hash(),
        start_loc,
        end_loc,
        EnumDefinition_::new(abilities, name, type_parameters, variants),
    ))
}

// VariantDecl: VariantDecl = {
//     <name_and_type_parameters:
//     NameAndTypeFormals> "{" <data: Comma<FieldDecl>> "}"
//     => { ... }
// }
fn parse_variant_decl(
    tokens: &mut Lexer,
) -> Result<VariantDefinition, ParseError<Loc, anyhow::Error>> {
    let start_loc = tokens.start_loc();

    let name = parse_name(tokens)?;

    consume_token(tokens, Tok::LBrace)?;
    let fields = parse_comma_list(tokens, &[Tok::RBrace], parse_field_decl, true)?;
    consume_token(tokens, Tok::RBrace)?;

    let end_loc = tokens.previous_end_loc();
    Ok(spanned(
        tokens.file_hash(),
        start_loc,
        end_loc,
        VariantDefinition_::new(name, fields),
    ))
}

// ModuleIdent: ModuleIdent = {
//     <a: AccountAddress> "." <m: ModuleName> => ModuleIdent::new(m, a),
// }

fn parse_module_ident(tokens: &mut Lexer) -> Result<ModuleIdent, ParseError<Loc, anyhow::Error>> {
    if tokens.peek() == Tok::DotNameValue {
        let start_loc = current_token_loc(tokens);
        let module_dot_name = parse_dot_name(tokens)?;
        let v: Vec<&str> = module_dot_name.split('.').collect();
        assert!(v.len() == 2);
        let address = parse_address_literal(tokens, v[0], start_loc)?;
        return Ok(ModuleIdent::new(ModuleName(Symbol::from(v[1])), address));
    }
    let a = parse_account_address(tokens)?;
    consume_token(tokens, Tok::Period)?;
    let m = parse_module_name(tokens)?;
    Ok(ModuleIdent::new(m, a))
}

// FriendDecl: ModuleIdent = {
//     "friend" <ident: ModuleIdent> ";" => { ... }
// }

fn parse_friend_decl(tokens: &mut Lexer) -> Result<ModuleIdent, ParseError<Loc, anyhow::Error>> {
    consume_token(tokens, Tok::Friend)?;
    let ident = parse_module_ident(tokens)?;
    consume_token(tokens, Tok::Semicolon)?;
    Ok(ident)
}

// ImportAlias: ModuleName = {
//     "as" <alias: ModuleName> => { ... }
// }

fn parse_import_alias(tokens: &mut Lexer) -> Result<ModuleName, ParseError<Loc, anyhow::Error>> {
    consume_token(tokens, Tok::As)?;
    let alias = parse_module_name(tokens)?;
    if alias == ModuleName::module_self() {
        panic!(
            "Invalid use of reserved module alias '{}'",
            ModuleName::self_name()
        );
    }
    Ok(alias)
}

// ImportDecl: ImportDefinition = {
//     "import" <ident: ModuleIdent> <alias: ImportAlias?> ";" => { ... }
// }

fn parse_import_decl(
    tokens: &mut Lexer,
) -> Result<ImportDefinition, ParseError<Loc, anyhow::Error>> {
    consume_token(tokens, Tok::Import)?;
    let ident = parse_module_ident(tokens)?;
    let alias = if tokens.peek() == Tok::As {
        Some(parse_import_alias(tokens)?)
    } else {
        None
    };
    consume_token(tokens, Tok::Semicolon)?;
    Ok(ImportDefinition::new(ident, alias))
}

// pub Module : ModuleDefinition = {
//     ["unpublishable"]
//     "module" <n: Name> "{"
//         <friends: (FriendDecl)*>
//         <imports: (ImportDecl)*>
//         <structs: (StructDecl)*>
//         <enums: (EnumDecl)*>
//         <functions: (FunctionDecl)*>
//     "}" =>? ModuleDefinition::new(n, imports, structs, functions),
// }

fn is_struct_decl(tokens: &mut Lexer) -> Result<bool, ParseError<Loc, anyhow::Error>> {
    let t = tokens.peek();
    Ok(t == Tok::Struct || (t == Tok::Native && tokens.lookahead()? == Tok::Struct))
}

fn is_enum_decl(tokens: &mut Lexer) -> bool {
    tokens.peek() == Tok::Enum
}

fn parse_module(tokens: &mut Lexer) -> Result<ModuleDefinition, ParseError<Loc, anyhow::Error>> {
    let publishable =
        if matches!(tokens.peek(), Tok::NameValue) && tokens.content() == "unpublishable" {
            tokens.advance()?;
            false
        } else {
            true
        };
    let start_loc = tokens.start_loc();
    consume_token(tokens, Tok::Module)?;
    let identifier = parse_module_ident(tokens)?;
    consume_token(tokens, Tok::LBrace)?;

    let mut friends = vec![];
    while tokens.peek() == Tok::Friend {
        friends.push(parse_friend_decl(tokens)?);
    }

    let mut imports = vec![];
    while tokens.peek() == Tok::Import {
        imports.push(parse_import_decl(tokens)?);
    }

    let mut structs: Vec<StructDefinition> = vec![];
    while is_struct_decl(tokens)? {
        structs.push(parse_struct_decl(tokens)?);
    }

    let mut enums: Vec<EnumDefinition> = vec![];
    while is_enum_decl(tokens) {
        enums.push(parse_enum_decl(tokens)?);
    }

    let mut functions: Vec<(FunctionName, Function)> = vec![];
    while tokens.peek() != Tok::RBrace {
        functions.push(parse_function_decl(tokens)?);
    }
    tokens.advance()?; // consume the RBrace
    let end_loc = tokens.previous_end_loc();
    let loc = make_loc(tokens.file_hash(), start_loc, end_loc);

    Ok(ModuleDefinition::new(
        None,
        loc,
        publishable,
        identifier,
        friends,
        imports,
        vec![],
        structs,
        enums,
        vec![],
        functions,
    ))
}

// pub ScriptOrModule: ScriptOrModule = {
//     <s: Script> => ScriptOrModule::Script(s),
//     <m: Module> => ScriptOrModule::Module(m),
// }

pub fn parse_module_string(
    input: &str,
) -> Result<ModuleDefinition, ParseError<Loc, anyhow::Error>> {
    parse_module_string_with_named_addresses(input, &BTreeMap::new())
}

pub fn parse_module_string_with_named_addresses(
    input: &str,
    named_addresses: &BTreeMap<String, AccountAddress>,
) -> Result<ModuleDefinition, ParseError<Loc, anyhow::Error>> {
    let file_hash = FileHash::new(input);
    let mut tokens = Lexer::new(file_hash, input, named_addresses);
    tokens.advance()?;
    let unit = parse_module(&mut tokens)?;
    consume_token(&mut tokens, Tok::EOF)?;
    Ok(unit)
}
