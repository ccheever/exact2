//! Pure row projections, recognized once per binding in the immutable plan.
//! Only the expression kind is retained; operands stay in its existing bytecode.
//! A prepared kind is evaluated with the same code it was recognized from.
//! Runtime scopes, field kinds, and string-pool accesses stay checked.
use super::{Env, Frame, Opcode, Scratch, Trap};
use exact_plan::Value;

mod conditional;

#[derive(Debug, Clone, Copy)]
pub(crate) enum Prepared {
    General,
    Item,
    Bound,
    ItemDefault,
    BoundDefault,
    NumberChoice,
    StringChoice,
    NumberCascade,
    StringPair,
}

impl Prepared {
    pub(crate) fn new(code: &[u8]) -> Self {
        let Some(&[load, _, _, field, _, _]) = code.get(..6) else {
            return Self::General;
        };
        if (load != Opcode::LoadItem as u8 && load != Opcode::LoadBound as u8)
            || field != Opcode::Field as u8
        {
            return Self::General;
        }
        let bound = load == Opcode::LoadBound as u8;
        if code.get(6..) == Some(&[Opcode::Return as u8]) {
            if bound {
                Self::Bound
            } else {
                Self::Item
            }
        } else if string_default(code).is_some() {
            if bound {
                Self::BoundDefault
            } else {
                Self::ItemDefault
            }
        } else {
            conditional::recognize(code)
        }
    }

    #[inline]
    pub(crate) fn eval(
        self,
        code: &[u8],
        env: &Env<'_>,
        frames: &[Frame],
        scratch: &mut Scratch,
    ) -> Result<Value, Trap> {
        if !matches!(self, Self::General) {
            // Recognition checked the prefix and complete body at the immutable site.
            let value = field(code, frames, 0)?;
            if matches!(
                self,
                Self::NumberChoice | Self::StringChoice | Self::NumberCascade | Self::StringPair
            ) {
                return conditional::eval(self, code, env, frames, value);
            }
            if matches!(self, Self::Item | Self::Bound) {
                return Ok(value.clone());
            }
            if value.is_str() {
                let literal = u32::from_le_bytes([code[11], code[12], code[13], code[14]]);
                if let Some(literal) = env.strings.get(literal as usize).filter(|v| v.is_str()) {
                    if value != literal {
                        return Ok(value.clone());
                    }
                    let replacement = u32::from_le_bytes([code[22], code[23], code[24], code[25]]);
                    if let Some(replacement) = env.strings.get(replacement as usize) {
                        return Ok(replacement.clone());
                    }
                }
            }
        }
        let env = Env { frames, ..*env };
        super::eval_value_reusing(code, &env, scratch)
    }
}

pub(super) fn string_default(code: &[u8]) -> Option<(u32, u32)> {
    use Opcode::{BindLocal, DropLocal, Eq, Jump, JumpIfFalse, LoadLocal, Return, Str};
    let tail: &[u8; 30] = code.get(6..)?.try_into().ok()?;
    if tail[..5] != [BindLocal as u8, LoadLocal as u8, 0, 0, Str as u8]
        || tail[9..16] != [Eq as u8, JumpIfFalse as u8, 31, 0, 0, 0, Str as u8]
        || tail[20..]
            != [
                Jump as u8,
                34,
                0,
                0,
                0,
                LoadLocal as u8,
                0,
                0,
                DropLocal as u8,
                Return as u8,
            ]
    {
        return None;
    }
    let literal = u32::from_le_bytes(tail[5..9].try_into().ok()?);
    let replacement = u32::from_le_bytes(tail[16..20].try_into().ok()?);
    Some((literal, replacement))
}

#[inline]
fn field<'a>(code: &[u8], frames: &'a [Frame], pc: usize) -> Result<&'a Value, Trap> {
    let depth = u16::from_le_bytes([code[pc + 1], code[pc + 2]]);
    let index = u16::from_le_bytes([code[pc + 4], code[pc + 5]]);
    let frame = super::frame_at(frames, depth as usize, pc)?;
    let value = if code[pc] == Opcode::LoadBound as u8 {
        &frame.bound
    } else {
        &frame.item
    };
    let value = value.as_ref().ok_or(Trap::BadScope { pc, depth })?;
    match value {
        Value::Record(fields) => fields
            .get(index as usize)
            .ok_or(Trap::BadField { pc: pc + 3, index }),
        _ => Err(Trap::TypeMismatch {
            pc: pc + 3,
            op: Opcode::Field,
        }),
    }
}
