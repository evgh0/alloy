#version 300 es
precision highp float;

in vec3 a_position;
in vec3 a_normal;
in vec3 a_instance_position;

flat out vec3 v_face_normal;

layout(std140) uniform Camera {
    vec4 camera_position;
    vec4 camera_target;
    vec4 camera_up;
    vec4 camera_projection;
};

layout(std140) uniform MeshTransform {
    vec4 mesh_position;
    vec4 mesh_scale;
    vec4 mesh_rotation;
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

vec3 quat_rotate(vec4 q, vec3 v) {
    vec3 t = 2.0 * cross(q.xyz, v);
    return v + q.w * t + cross(q.xyz, t);
}

void main() {
    vec4 rotation = normalize(mesh_rotation);
    vec3 scaled_position = a_position * mesh_scale.xyz;
    vec3 world_position = quat_rotate(rotation, scaled_position) + mesh_position.xyz + a_instance_position;

    vec3 normal_scale = max(abs(mesh_scale.xyz), vec3(0.0001));
    vec3 world_normal = normalize(quat_rotate(rotation, a_normal / normal_scale));

    mat4 view = look_at(camera_position.xyz, camera_target.xyz, camera_up.xyz);
    mat4 proj = perspective(camera_projection.x, camera_projection.y, camera_projection.z, camera_projection.w);
    v_face_normal = world_normal;
    gl_Position = proj * view * vec4(world_position, 1.0);
}
