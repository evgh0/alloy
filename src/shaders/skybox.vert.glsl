#version 300 es
precision highp float;

in vec3 a_position;

out vec3 v_direction;

layout(std140) uniform Camera {
    vec4 camera_position;
    vec4 camera_target;
    vec4 camera_up;
    vec4 camera_projection;
};

mat4 look_at(vec3 eye, vec3 center, vec3 up_vector) {
    vec3 f = normalize(center - eye);
    vec3 s = normalize(cross(f, up_vector));
    vec3 u = cross(s, f);

    return mat4(
        vec4(s, 0.0),
        vec4(u, 0.0),
        vec4(-f, 0.0),
        vec4(-dot(s, eye), -dot(u, eye), dot(f, eye), 1.0)
    );
}

mat4 perspective(float fovy, float aspect, float near_plane, float far_plane) {
    float f = 1.0 / tan(fovy * 0.5);
    float range_inv = 1.0 / (near_plane - far_plane);

    return mat4(
        vec4(f / aspect, 0.0, 0.0, 0.0),
        vec4(0.0, f, 0.0, 0.0),
        vec4(0.0, 0.0, (far_plane + near_plane) * range_inv, -1.0),
        vec4(0.0, 0.0, (2.0 * far_plane * near_plane) * range_inv, 0.0)
    );
}

void main() {
    vec3 direction = a_position;
    v_direction = direction;

    mat4 view = look_at(camera_position.xyz, camera_target.xyz, camera_up.xyz);
    mat4 view_rotation = mat4(mat3(view));
    mat4 proj = perspective(camera_projection.x, camera_projection.y, camera_projection.z, camera_projection.w);
    vec4 clip_position = proj * view_rotation * vec4(direction, 1.0);

    gl_Position = vec4(clip_position.xy, clip_position.w, clip_position.w);
}
