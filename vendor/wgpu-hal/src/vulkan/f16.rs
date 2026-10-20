// EXACT (EXACT-PATCHES.md, 11): Naga 30 overdeclares 16-bit uniform storage.
use alloc::vec::Vec;
use naga::back::spv::Capability;
use naga::{AddressSpace, Handle, Module, Scalar, Type, TypeInner};

/// The 16-bit grants a device's Naga capability allowlist carries:
/// `Float16` with `SHADER_F16`, and the uniform buffer grant only where the
/// adapter has `uniformAndStorageBuffer16BitAccess` (the translator then
/// leaves the module as Naga wrote it).
pub(super) fn allow(capabilities: &mut Vec<Capability>, shader_f16: bool, uniform_16bit: bool) {
    if shader_f16 {
        capabilities.push(Capability::Float16);
    }
    if uniform_16bit {
        capabilities.push(Capability::UniformAndStorageBuffer16BitAccess);
    }
}

fn has_16bit(module: &Module, ty: Handle<Type>) -> bool {
    match &module.types[ty].inner {
        TypeInner::Scalar(s)
        | TypeInner::Vector { scalar: s, .. }
        | TypeInner::Matrix { scalar: s, .. }
        | TypeInner::Atomic(s) => s.width == 2,
        TypeInner::Array { base, .. } | TypeInner::BindingArray { base, .. } => {
            has_16bit(module, *base)
        }
        TypeInner::Struct { members, .. } => members.iter().any(|m| has_16bit(module, m.ty)),
        _ => false,
    }
}

/// `OpCapability`.
const OP_CAPABILITY: u32 = 17;

/// Each instruction's opcode and operands, after the five-word header.
fn instructions(words: &[u32]) -> impl Iterator<Item = (u32, &[u32])> {
    let mut at = 5;
    core::iter::from_fn(move || {
        let count = (*words.get(at)? >> 16) as usize;
        let instruction = words.get(at..at + count.max(1))?;
        at += count.max(1);
        Some((instruction[0] & 0xffff, &instruction[1..]))
    })
}

/// Make `words`, Naga's translation of `module`, declare no 16-bit uniform
/// storage, for a device without it; refuse a module that needs it.
/// Naga's `make_scalar` declares `StorageBuffer16BitAccess` and
/// `UniformAndStorageBuffer16BitAccess` for any 16-bit type, past the
/// allowlist; only the uniform grant is removed, and the result must still
/// declare what the module uses (`Float16` for a half type,
/// `StorageBuffer16BitAccess` for half data in a storage buffer) and no
/// 16-bit push constants, which the device never enables.
pub(super) fn restrict_uniforms(module: &Module, words: &mut Vec<u32>) -> Result<(), &'static str> {
    let in_space = |space: fn(AddressSpace) -> bool| {
        module
            .global_variables
            .iter()
            .any(|(_, var)| space(var.space) && has_16bit(module, var.ty))
    };
    if in_space(|s| s == AddressSpace::Uniform) {
        return Err("this Vulkan adapter supports f16 arithmetic and storage buffers, but not 16-bit uniform buffers");
    }
    if in_space(|s| s == AddressSpace::Immediate) {
        return Err("16-bit immediate data is not supported");
    }
    let mut at = 5; // SPIR-V header.
    while at < words.len() {
        let count = (words[at] >> 16) as usize;
        if words[at] & 0xffff == OP_CAPABILITY
            && words[at + 1] == Capability::UniformAndStorageBuffer16BitAccess as u32
        {
            words.drain(at..at + count);
        } else {
            at += count.max(1);
        }
    }
    let declared = |capability: Capability| {
        instructions(words).any(|(op, operands)| {
            op == OP_CAPABILITY && operands.first() == Some(&(capability as u32))
        })
    };
    let half = module.types.iter().any(|(_, ty)| {
        matches!(
            ty.inner,
            TypeInner::Scalar(Scalar::F16)
                | TypeInner::Vector {
                    scalar: Scalar::F16,
                    ..
                }
                | TypeInner::Matrix {
                    scalar: Scalar::F16,
                    ..
                }
        )
    });
    if half && !declared(Capability::Float16) {
        return Err("a half-precision module without the Float16 capability");
    }
    if in_space(|s| matches!(s, AddressSpace::Storage { .. }))
        && !declared(Capability::StorageBuffer16BitAccess)
    {
        return Err("16-bit storage buffer data without the StorageBuffer16BitAccess capability");
    }
    if declared(Capability::StoragePushConstant16)
        || declared(Capability::UniformAndStorageBuffer16BitAccess)
    {
        return Err("a 16-bit capability this device does not enable");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use naga::back::spv;

    /// What Naga writes for `source` under the allowlist of an adapter with
    /// `shaderFloat16` and `storageBuffer16BitAccess` but no 16-bit uniform
    /// buffers (Adreno 620), as the device builds it (`adapter.rs`): the
    /// always-available Vulkan set and [`allow`]'s grants, with stage I/O
    /// in 16 bits or not.
    fn translate(source: &str, io16: bool) -> (Module, Result<Vec<u32>, spv::Error>) {
        let module = naga::front::wgsl::parse_str(source).unwrap();
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap();
        let mut capabilities = vec![
            Capability::Shader,
            Capability::Matrix,
            Capability::Sampled1D,
            Capability::Image1D,
            Capability::ImageQuery,
            Capability::DerivativeControl,
            Capability::StorageImageExtendedFormats,
        ];
        allow(&mut capabilities, true, false);
        let options = spv::Options {
            lang_version: (1, 3),
            capabilities: Some(capabilities.into_iter().collect()),
            use_storage_input_output_16: io16,
            ..Default::default()
        };
        let words = spv::write_vec(&module, &info, &options, None);
        (module, words)
    }

    fn declares(words: &[u32], capability: Capability) -> bool {
        instructions(words).any(|(op, operands)| {
            op == OP_CAPABILITY && operands.first() == Some(&(capability as u32))
        })
    }

    #[test]
    fn half_data_keeps_its_grants_and_loses_only_the_uniform_one() {
        let shaders = [
            ("private", "var<private> value: vec4<f16>; @compute @workgroup_size(1) fn main() { value = vec4<f16>(0.5); }"),
            ("workgroup", "var<workgroup> value: array<f16, 4>; @compute @workgroup_size(1) fn main() { value[0] = f16(0.5); }"),
            ("storage", "@group(0) @binding(0) var<storage, read_write> value: array<vec4<f16>>; @compute @workgroup_size(1) fn main() { value[0] = vec4<f16>(0.5); }"),
            ("stage io", "@vertex fn vs(@location(0) a: vec4<f16>) -> @builtin(position) vec4<f32> { return vec4<f32>(a); }"),
        ];
        for (what, body) in shaders {
            for io16 in [false, true] {
                let (module, words) = translate(&format!("enable f16; {body}"), io16);
                let mut words = words.unwrap_or_else(|e| panic!("{what}: {e}"));
                restrict_uniforms(&module, &mut words).unwrap_or_else(|e| panic!("{what}: {e}"));
                assert!(declares(&words, Capability::Float16), "{what}");
                assert!(
                    declares(&words, Capability::StorageBuffer16BitAccess),
                    "{what}"
                );
                assert!(
                    !declares(&words, Capability::UniformAndStorageBuffer16BitAccess),
                    "{what}"
                );
                assert!(
                    !declares(&words, Capability::StoragePushConstant16),
                    "{what}"
                );
            }
        }
    }

    #[test]
    fn half_uniform_and_immediate_data_are_refused() {
        for ty in ["vec4<f16>", "array<vec4<f16>, 2>", "Nested"] {
            let source = format!("enable f16; struct Nested {{ v: array<vec4<f16>, 2> }} @group(0) @binding(0) var<uniform> value: {ty};");
            let module = naga::front::wgsl::parse_str(&source).unwrap();
            assert!(restrict_uniforms(&module, &mut vec![]).is_err(), "{ty}");
        }
        // Naga's validator refuses these too; this does not depend on it.
        let module =
            naga::front::wgsl::parse_str("enable f16; var<immediate> value: vec4<f16>;").unwrap();
        assert!(restrict_uniforms(&module, &mut vec![]).is_err());
    }

    #[test]
    fn a_module_that_lost_a_grant_it_needs_is_refused() {
        let (module, words) = translate(
            "enable f16; @group(0) @binding(0) var<storage, read_write> value: array<f16>; @compute @workgroup_size(1) fn main() { value[0] = f16(0.5); }",
            false,
        );
        let mut words = words.unwrap();
        // As if Naga had declared only the uniform grant, which implies the
        // storage one: removing it leaves the storage buffer undeclared.
        let mut at = 5;
        while at < words.len() {
            let count = (words[at] >> 16) as usize;
            if words[at] & 0xffff == OP_CAPABILITY
                && words[at + 1] == Capability::StorageBuffer16BitAccess as u32
            {
                words.drain(at..at + count);
            } else {
                at += count;
            }
        }
        assert!(restrict_uniforms(&module, &mut words).is_err());
    }
}
