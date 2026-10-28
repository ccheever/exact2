use super::*;
use windows::core::s;

pub(super) struct Pipeline {
    pub vertex: ID3D11VertexShader,
    pub pixel: ID3D11PixelShader,
    pub layout: ID3D11InputLayout,
    pub blend: ID3D11BlendState,
    pub replace: ID3D11BlendState,
    pub raster: ID3D11RasterizerState,
    pub sampler: ID3D11SamplerState,
    pub repeat: ID3D11SamplerState,
    pub viewport: ID3D11Buffer,
}

pub(super) fn open(warp: bool) -> Result<(ID3D11Device, ID3D11DeviceContext, String)> {
    let (mut device, mut context) = (None, None);
    // SAFETY: the API writes owned COM references; the feature list and outputs
    // remain live for the call. BGRA is required by Direct2D (LLP 1149 D5.2).
    unsafe {
        D3D11CreateDevice(
            None,
            if warp {
                D3D_DRIVER_TYPE_WARP
            } else {
                D3D_DRIVER_TYPE_HARDWARE
            },
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )?;
        let device =
            device.ok_or_else(|| Error::Device("D3D11CreateDevice returned no device".into()))?;
        let dxgi: IDXGIDevice = device.cast()?;
        let adapter = dxgi.GetAdapter()?.GetDesc()?;
        let end = adapter
            .Description
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(adapter.Description.len());
        Ok((
            device,
            context.unwrap(),
            String::from_utf16_lossy(&adapter.Description[..end]),
        ))
    }
}

pub(super) fn pipeline(device: &ID3D11Device) -> Result<Pipeline> {
    let vs = include_bytes!(concat!(env!("OUT_DIR"), "/ink.vs.dxbc"));
    let ps = include_bytes!(concat!(env!("OUT_DIR"), "/ink.ps.dxbc"));
    if vs.is_empty() || ps.is_empty() {
        return Err(Error::Unsupported(
            "Direct3D 11 DXBC is absent: rebuild exact-ink with Windows SDK fxc on PATH",
        ));
    }
    let (mut vertex, mut pixel, mut layout) = (None, None, None);
    let attributes: Vec<_> = (0..12)
        .map(|i| D3D11_INPUT_ELEMENT_DESC {
            SemanticName: s!("LOC"),
            SemanticIndex: i,
            Format: if i == 2 || i == 7 {
                DXGI_FORMAT_R32G32B32A32_UINT
            } else {
                DXGI_FORMAT_R32G32B32A32_FLOAT
            },
            InputSlot: 0,
            AlignedByteOffset: i * 16,
            InputSlotClass: D3D11_INPUT_PER_INSTANCE_DATA,
            InstanceDataStepRate: 1,
        })
        .collect();
    // SAFETY: DXBC is build-generated; descriptors cover the core's 192-byte
    // Instance. COM output pointers are initialized and owned by the result.
    unsafe {
        device.CreateVertexShader(vs, None, Some(&mut vertex))?;
        device.CreatePixelShader(ps, None, Some(&mut pixel))?;
        device.CreateInputLayout(&attributes, vs, Some(&mut layout))?;
        let blend = |enabled: bool| -> Result<ID3D11BlendState> {
            let mut desc = D3D11_BLEND_DESC::default();
            desc.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
                BlendEnable: enabled.into(),
                SrcBlend: D3D11_BLEND_ONE,
                DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOp: D3D11_BLEND_OP_ADD,
                SrcBlendAlpha: D3D11_BLEND_ONE,
                DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOpAlpha: D3D11_BLEND_OP_ADD,
                RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
            };
            let mut state = None;
            device.CreateBlendState(&desc, Some(&mut state))?;
            Ok(state.unwrap())
        };
        let mut raster = None;
        device.CreateRasterizerState(
            &D3D11_RASTERIZER_DESC {
                FillMode: D3D11_FILL_SOLID,
                CullMode: D3D11_CULL_NONE,
                DepthClipEnable: true.into(),
                ScissorEnable: true.into(),
                ..Default::default()
            },
            Some(&mut raster),
        )?;
        let sampler = |repeat| -> Result<ID3D11SamplerState> {
            let address = if repeat {
                D3D11_TEXTURE_ADDRESS_WRAP
            } else {
                D3D11_TEXTURE_ADDRESS_CLAMP
            };
            let mut state = None;
            device.CreateSamplerState(
                &D3D11_SAMPLER_DESC {
                    Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                    AddressU: address,
                    AddressV: address,
                    AddressW: address,
                    MaxLOD: f32::MAX,
                    ComparisonFunc: D3D11_COMPARISON_NEVER,
                    ..Default::default()
                },
                Some(&mut state),
            )?;
            Ok(state.unwrap())
        };
        let viewport = make_buffer(device, 16, D3D11_BIND_CONSTANT_BUFFER)?;
        Ok(Pipeline {
            vertex: vertex.unwrap(),
            pixel: pixel.unwrap(),
            layout: layout.unwrap(),
            blend: blend(true)?,
            replace: blend(false)?,
            raster: raster.unwrap(),
            sampler: sampler(false)?,
            repeat: sampler(true)?,
            viewport,
        })
    }
}

pub(super) fn make_buffer(
    device: &ID3D11Device,
    size: u32,
    bind: D3D11_BIND_FLAG,
) -> Result<ID3D11Buffer> {
    let mut raw = None;
    unsafe {
        device.CreateBuffer(
            &D3D11_BUFFER_DESC {
                ByteWidth: size,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: bind.0 as u32,
                ..Default::default()
            },
            None,
            Some(&mut raw),
        )?;
    }
    Ok(raw.unwrap())
}

pub(super) fn texture(device: &ID3D11Device, desc: TextureDesc) -> Result<Texture> {
    let format = if desc.format == Format::Alpha {
        DXGI_FORMAT_R8_UNORM
    } else {
        DXGI_FORMAT_R8G8B8A8_UNORM
    };
    let mut raw = None;
    unsafe {
        device.CreateTexture2D(
            &D3D11_TEXTURE2D_DESC {
                Width: desc.width,
                Height: desc.height,
                MipLevels: desc.levels(),
                ArraySize: 1,
                Format: format,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: (D3D11_BIND_SHADER_RESOURCE | D3D11_BIND_RENDER_TARGET).0 as u32,
                ..Default::default()
            },
            None,
            Some(&mut raw),
        )?;
    }
    let raw = raw.unwrap();
    let mut view = None;
    unsafe {
        device.CreateShaderResourceView(&raw, None, Some(&mut view))?;
    }
    Ok(Texture {
        raw,
        view: view.unwrap(),
        desc,
        last: Submission(0),
    })
}
