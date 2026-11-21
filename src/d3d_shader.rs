use crate::shaders::Shader;
use std::ffi::CString;
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::Graphics::Direct3D::{Fxc::D3DCompile, ID3DBlob, ID3DInclude};
use windows::core::{Error, PCSTR, Result};

pub(crate) fn compile(shader: Shader) -> Result<ID3DBlob> {
    let entry = CString::new(shader.entry).unwrap();
    let profile = CString::new(shader.profile).unwrap();
    let mut code = None;
    let mut diagnostics = None;
    // Inputs live for the call and the shader expects no includes.
    let result = unsafe {
        D3DCompile(
            shader.source.as_ptr().cast(),
            shader.source.len(),
            PCSTR::null(),
            None,
            None::<&ID3DInclude>,
            PCSTR(entry.as_ptr().cast()),
            PCSTR(profile.as_ptr().cast()),
            0,
            0,
            &mut code,
            Some(&mut diagnostics),
        )
    };
    result.map_err(|error| diagnostic(error, diagnostics))?;
    code.ok_or_else(|| Error::from_hresult(E_FAIL))
}

pub(crate) fn blob_bytes(blob: &ID3DBlob) -> &[u8] {
    // The compiler owns this storage for the lifetime of the blob.
    unsafe { std::slice::from_raw_parts(blob.GetBufferPointer().cast(), blob.GetBufferSize()) }
}

pub(crate) fn diagnostic(error: Error, diagnostics: Option<ID3DBlob>) -> Error {
    match diagnostics {
        Some(blob) => {
            let message = String::from_utf8_lossy(blob_bytes(&blob));
            Error::new(error.code(), message.trim_end_matches('\0'))
        }
        None => error,
    }
}

#[cfg(any(feature = "d3d10", feature = "d3d11", feature = "d3d12"))]
pub(crate) fn out<T>(create: impl FnOnce(&mut Option<T>) -> Result<()>) -> Result<T> {
    let mut value = None;
    create(&mut value)?;
    value.ok_or_else(|| Error::from_hresult(E_FAIL))
}

#[cfg(any(feature = "d3d10", feature = "d3d11", feature = "d3d12"))]
pub(crate) fn is_srgb(format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT) -> bool {
    use windows::Win32::Graphics::Dxgi::Common::*;
    matches!(
        format,
        DXGI_FORMAT_R8G8B8A8_UNORM_SRGB
            | DXGI_FORMAT_B8G8R8A8_UNORM_SRGB
            | DXGI_FORMAT_B8G8R8X8_UNORM_SRGB
    )
}

#[cfg(any(feature = "d3d10", feature = "d3d11"))]
pub(crate) fn is_unorm(format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT) -> bool {
    use windows::Win32::Graphics::Dxgi::Common::*;
    matches!(
        format,
        DXGI_FORMAT_R8G8B8A8_UNORM | DXGI_FORMAT_B8G8R8A8_UNORM | DXGI_FORMAT_B8G8R8X8_UNORM
    )
}
