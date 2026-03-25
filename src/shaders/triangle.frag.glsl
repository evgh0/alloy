#version 300 es
precision mediump float;

flat in vec3 v_face_normal;

out vec4 out_color;

void main() {
    vec3 light_dir = normalize(vec3(0.4, 0.7, 1.0));
    float diffuse = max(dot(normalize(v_face_normal), light_dir), 0.0);
    vec3 base_color = vec3(0.1, 0.7, 0.9);
    vec3 shaded = base_color * (0.25 + 0.75 * diffuse);
    out_color = vec4(shaded, 1.0);
}
