//! Copy borrowed CEF pixels to application storage before releasing the callback.
use crate::painter::{AlphaSource, bind_texture};
use crate::shared::{Importer, acquire_barrier};
use crate::{FORMAT, SharedTexture, Surfaces};

impl Surfaces {
    pub(crate) fn capture(&self, frame: &SharedTexture) -> Option<wgpu::Texture> {
        let imported = Importer::new()
            .import(&self.device, frame)
            .map_err(|error| {
                tracing::error!("CEF frame capture: {error}");
            })
            .ok()?;
        let size = imported.texture.size();
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("owned CEF frame"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let bind = bind_texture(self, "CEF capture source", &imported.texture);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let gate = self.submit_gate.read();
        if let Some(image) = imported.acquire {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
            acquire_barrier(&self.device, &mut encoder, image);
            self.queue.submit([encoder.finish()]);
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("CEF snapshot"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(self.pipeline(AlphaSource::Premultiplied));
            pass.set_bind_group(0, &bind, &[]);
            pass.draw(0..3, 0..1);
        }
        let submission = self.queue.submit([encoder.finish()]);
        drop(gate);
        // Submission is asynchronous. Returning early lets CEF overwrite a
        // texture the GPU is still reading. Wait for this copy, not later work.
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: None,
            })
            .map_err(|error| tracing::error!("CEF capture completion: {error}"))
            .ok()?;
        Some(texture)
    }
}
