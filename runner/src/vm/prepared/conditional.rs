//! Literal choices from one or two projected booleans. Recognition checks
//! the whole body once; constants and field operands stay in the bytecode.
use super::{Env, Frame, Opcode, Prepared, Trap, Value};

pub(super) fn recognize(code: &[u8]) -> Prepared {
    use Opcode::{
        BindLocal, DropLocal, Field, Jump, JumpIfFalse, LoadBound, LoadItem, LoadLocal, Number,
        Return, Str,
    };
    let jump = |at: usize, op: Opcode, target: u8| {
        code.get(at..at + 5) == Some(&[op as u8, target, 0, 0, 0])
    };
    if code.len() == 35
        && jump(6, JumpIfFalse, 25)
        && code[11] == Number as u8
        && jump(20, Jump, 34)
        && code[25] == Number as u8
        && code[34] == Return as u8
    {
        return Prepared::NumberChoice;
    }
    if code.len() == 27
        && jump(6, JumpIfFalse, 21)
        && code[11] == Str as u8
        && jump(16, Jump, 26)
        && code[21] == Str as u8
        && code[26] == Return as u8
    {
        return Prepared::StringChoice;
    }
    let Some(&[bind, load, _, _, field, _, _, bind2]) = code.get(6..14) else {
        return Prepared::General;
    };
    if bind != BindLocal as u8
        || bind2 != BindLocal as u8
        || (load != LoadItem as u8 && load != LoadBound as u8)
        || field != Field as u8
        || code.get(14..17) != Some(&[LoadLocal as u8, 0, 0])
    {
        return Prepared::General;
    }
    if code.len() == 70
        && jump(17, JumpIfFalse, 36)
        && code[22] == Number as u8
        && jump(31, Jump, 67)
        && code[36..39] == [LoadLocal as u8, 1, 0]
        && jump(39, JumpIfFalse, 58)
        && code[44] == Number as u8
        && jump(53, Jump, 67)
        && code[58] == Number as u8
        && code[67..] == [DropLocal as u8, DropLocal as u8, Return as u8]
    {
        return Prepared::NumberCascade;
    }
    if code.len() == 76
        && jump(17, JumpIfFalse, 50)
        && code[22..25] == [LoadLocal as u8, 1, 0]
        && jump(25, JumpIfFalse, 40)
        && code[30] == Str as u8
        && jump(35, Jump, 45)
        && code[40] == Str as u8
        && jump(45, Jump, 73)
        && code[50..53] == [LoadLocal as u8, 1, 0]
        && jump(53, JumpIfFalse, 68)
        && code[58] == Str as u8
        && jump(63, Jump, 73)
        && code[68] == Str as u8
        && code[73..] == [DropLocal as u8, DropLocal as u8, Return as u8]
    {
        return Prepared::StringPair;
    }
    Prepared::General
}

pub(super) fn eval(
    kind: Prepared,
    code: &[u8],
    env: &Env<'_>,
    frames: &[Frame],
    first: &Value,
) -> Result<Value, Trap> {
    let boolean = |value: &Value, pc: usize| match value {
        Value::Bool(b) => Ok(*b),
        _ => Err(Trap::TypeMismatch {
            pc,
            op: Opcode::JumpIfFalse,
        }),
    };
    let number =
        |pc: usize| Value::Number(f64::from_le_bytes(code[pc + 1..pc + 9].try_into().unwrap()));
    let string = |pc: usize| {
        let index = u32::from_le_bytes(code[pc + 1..pc + 5].try_into().unwrap());
        env.strings
            .get(index as usize)
            .cloned()
            .ok_or(Trap::Malformed { pc })
    };
    match kind {
        Prepared::NumberChoice => Ok(number(if boolean(first, 6)? { 11 } else { 25 })),
        Prepared::StringChoice => string(if boolean(first, 6)? { 11 } else { 21 }),
        Prepared::NumberCascade => {
            // A lowered function reads both arguments before branching, even
            // when its first condition makes the second argument unused.
            let second = super::field(code, frames, 7)?;
            let pc = if boolean(first, 17)? {
                22
            } else if boolean(second, 39)? {
                44
            } else {
                58
            };
            Ok(number(pc))
        }
        Prepared::StringPair => {
            let second = super::field(code, frames, 7)?;
            string(if boolean(first, 17)? {
                if boolean(second, 25)? {
                    30
                } else {
                    40
                }
            } else if boolean(second, 53)? {
                58
            } else {
                68
            })
        }
        _ => unreachable!("recognized conditional kind"),
    }
}
