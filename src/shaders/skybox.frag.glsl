#version 300 es
precision mediump float;

in vec3 v_direction;

uniform samplerCube u_skybox;

out vec4 out_color;

void main() {
    out_color = texture(u_skybox, normalize(v_direction));
}
