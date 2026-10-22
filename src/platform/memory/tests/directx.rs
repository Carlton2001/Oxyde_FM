//! Isolated hardware probe using the texture upload/binding pattern of GPUI.
//! Measurements are diagnostics, not hardware-independent memory assertions.
use std::{
    thread,
    time::{Duration, Instant},
};
use windows::{
    core::Interface,
    Win32::{
        Foundation::HMODULE,
        Graphics::{
            Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0},
            Direct3D11::*,
            Dxgi::{
                Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC},
                IDXGIAdapter, IDXGIDevice3,
            },
        },
    },
};

fn sample(stage: &str) {
    let sample = crate::platform::platform()
        .process_memory_snapshot()
        .unwrap();
    println!(
        "graphics-memory: stage={stage}; private_bytes={}; working_set_bytes={}",
        sample.private_bytes, sample.working_set_bytes
    );
}

#[test]
#[ignore = "Hardware memory diagnostic; run alone with --nocapture"]
fn directx_image_upload_memory_lifecycle() {
    run(false);
}

#[test]
#[ignore = "Hardware memory diagnostic; run alone with --nocapture"]
fn directx_image_initial_data_memory_lifecycle() {
    run(true);
}

fn wait_gpu(device: &ID3D11Device, context: &ID3D11DeviceContext) {
    let mut query = None;
    unsafe {
        device
            .CreateQuery(
                &D3D11_QUERY_DESC {
                    Query: D3D11_QUERY_EVENT,
                    MiscFlags: 0,
                },
                Some(&mut query),
            )
            .unwrap();
        let query = query.unwrap();
        context.End(&query);
        context.Flush();
        let started = Instant::now();
        loop {
            let mut ready = 0u32;
            context
                .GetData(&query, Some((&mut ready as *mut u32).cast()), 4, 0)
                .unwrap();
            if ready != 0 {
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "GPU completion timed out"
            );
            thread::sleep(Duration::from_millis(1));
        }
    }
}

fn verify_pixels(
    device: &ID3D11Device,
    context: &ID3D11DeviceContext,
    texture: &ID3D11Texture2D,
    pixels: &[u8],
) {
    let description = D3D11_TEXTURE2D_DESC {
        Width: 8,
        Height: 8,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_STAGING,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        ..Default::default()
    };
    let mut staging = None;
    unsafe {
        device
            .CreateTexture2D(&description, None, Some(&mut staging))
            .unwrap();
        let staging = staging.unwrap();
        context.CopySubresourceRegion(
            &staging,
            0,
            0,
            0,
            0,
            texture,
            0,
            Some(&D3D11_BOX {
                left: 0,
                top: 0,
                front: 0,
                right: 8,
                bottom: 8,
                back: 1,
            }),
        );
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        context
            .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
            .unwrap();
        for row in 0..8usize {
            let actual = std::slice::from_raw_parts(
                (mapped.pData as *const u8).add(row * mapped.RowPitch as usize),
                8 * 4,
            );
            assert_eq!(
                actual,
                &pixels[row * 4000 * 4..row * 4000 * 4 + 8 * 4],
                "GPU pixel round-trip row {row}"
            );
        }
        context.Unmap(&staging, 0);
    }
}

fn run(initial_data: bool) {
    println!("graphics-memory: initial_data={initial_data}");
    sample("before-device");
    let mut device = None;
    let mut context = None;
    unsafe {
        D3D11CreateDevice(
            None::<&IDXGIAdapter>,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
        .unwrap();
    }
    let device = device.unwrap();
    let context = context.unwrap();
    {
        let dxgi: IDXGIDevice3 = device.cast().unwrap();
        let desc = unsafe { dxgi.GetAdapter().unwrap().GetDesc().unwrap() };
        println!(
            "graphics-memory: adapter={}; vendor_id={:#x}",
            String::from_utf16_lossy(&desc.Description).trim_end_matches('\0'),
            desc.VendorId
        );
    }
    sample("device-created");
    {
        let mut pixels = vec![0u8; 4000 * 4000 * 4];
        for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
            pixel.copy_from_slice(&[(index % 251) as u8, (index % 239) as u8, 120, 255]);
        }
        for iteration in 0..8 {
            pixels[0] = iteration;
            let description = D3D11_TEXTURE2D_DESC {
                Width: 4000,
                Height: 4000,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
                ..Default::default()
            };
            let mut texture = None;
            let mut view = None;
            unsafe {
                let initial = D3D11_SUBRESOURCE_DATA {
                    pSysMem: pixels.as_ptr().cast(),
                    SysMemPitch: 4000 * 4,
                    SysMemSlicePitch: 0,
                };
                device
                    .CreateTexture2D(
                        &description,
                        initial_data.then_some(&initial as *const _),
                        Some(&mut texture),
                    )
                    .unwrap();
                let texture = texture.unwrap();
                if !initial_data {
                    context.UpdateSubresource(
                        &texture,
                        0,
                        None,
                        pixels.as_ptr().cast(),
                        4000 * 4,
                        0,
                    );
                }
                device
                    .CreateShaderResourceView(&texture, None, Some(&mut view))
                    .unwrap();
                context.VSSetShaderResources(0, Some(&[view.clone()]));
                context.PSSetShaderResources(0, Some(&[view]));
                if iteration == 7 {
                    verify_pixels(&device, &context, &texture, &pixels);
                }
            }
            wait_gpu(&device, &context);
        }
    } // All source pixels and explicit texture/view COM references are released.
    wait_gpu(&device, &context);
    thread::sleep(Duration::from_millis(250));
    sample("uploads-dropped-still-bound");
    unsafe {
        context.VSSetShaderResources(0, Some(&[None]));
        context.PSSetShaderResources(0, Some(&[None]));
        context.Flush();
    }
    thread::sleep(Duration::from_millis(250));
    sample("unbound-and-flushed");
    thread::sleep(Duration::from_secs(30));
    sample("unbound-idle-30s");
    {
        let dxgi: IDXGIDevice3 = device.cast().unwrap();
        unsafe {
            dxgi.Trim();
        }
    }
    thread::sleep(Duration::from_millis(250));
    sample("dxgi-trimmed");
    unsafe {
        context.ClearState();
        context.Flush();
    }
    drop(context);
    drop(device);
    thread::sleep(Duration::from_millis(250));
    sample("device-dropped");
}
