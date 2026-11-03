//! Samples the portal render target in screen space.
//!
//! The portal camera is placed so that the destination plane projects onto the
//! same screen rectangle as the source portal (see `src/portal.rs`), which
//! means the fragment's own screen position is the correct lookup coordinate.
//!
//! The render target always has the same pixel size as the window, so
//! `textureDimensions` gives us everything needed to normalize the coordinate
//! and no view/uniform data is required.

import bevy_pbr::render::forward_io::VertexOutput;

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var portal_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var portal_sampler: sampler;

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(portal_texture));
    let uv = input.position.xy / size;

    // The portal camera renders with `Tonemapping::None`, so the image holds
    // linear values and the player's camera tone-maps the final frame.
    let color = textureSample(portal_texture, portal_sampler, uv);
    return vec4(color.rgb, 1.0);
}
