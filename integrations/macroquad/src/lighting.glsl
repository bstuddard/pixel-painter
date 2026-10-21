#version 100
// SPDX-License-Identifier: GPL-3.0-only
precision highp float;
varying highp vec2 uv;
uniform sampler2D Texture;
uniform sampler2D EmissionTexture;
uniform sampler2D NormalTexture;
uniform float Mode;
uniform vec3 Light;
uniform vec2 Flip;

float linear_channel(float c) {
    return c <= 0.04045 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4);
}
float srgb_channel(float c) {
    c = clamp(c, 0.0, 1.0);
    return c <= 0.0031308 ? c * 12.92 : 1.055 * pow(c, 1.0 / 2.4) - 0.055;
}
vec3 linear_rgb(vec3 c) {
    return vec3(linear_channel(c.r), linear_channel(c.g), linear_channel(c.b));
}
void main() {
    // All samplers share one frame rectangle. Normal samples remain linear data.
    vec4 color = texture2D(Texture, uv);
    vec4 emission = texture2D(EmissionTexture, uv);
    vec3 normal = 2.0 * texture2D(NormalTexture, uv).rgb - 1.0;
    normal.xy *= Flip;
    float diffuse = Mode < 2.5 ? 1.0 : 0.18 + 0.82 * max(dot(normalize(normal), Light), 0.0);
    // Exported emission alpha already contains color coverage. Convert back to
    // straight strength here; the output blend applies coverage exactly once.
    float strength = Mode < 1.5 || color.a == 0.0 ? 0.0 : emission.a / color.a;
    vec3 radiance = linear_rgb(color.rgb) * diffuse + linear_rgb(emission.rgb) * strength;
    gl_FragColor = vec4(srgb_channel(radiance.r), srgb_channel(radiance.g), srgb_channel(radiance.b), color.a);
}
