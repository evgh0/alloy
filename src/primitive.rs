fn push_vertex(vertices: &mut Vec<f32>, position: [f32; 3], normal: [f32; 3]) {
    vertices.extend_from_slice(&[
        position[0],
        position[1],
        position[2],
        normal[0],
        normal[1],
        normal[2],
    ]);
}

fn push_face(
    vertices: &mut Vec<f32>,
    a: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
    d: [f32; 3],
    normal: [f32; 3],
) {
    push_vertex(vertices, a, normal);
    push_vertex(vertices, b, normal);
    push_vertex(vertices, c, normal);
    push_vertex(vertices, a, normal);
    push_vertex(vertices, c, normal);
    push_vertex(vertices, d, normal);
}

pub fn cube() -> Vec<f32> {
    let mut vertices: Vec<f32> = Vec::with_capacity(36 * 6);

    let p000 = [-0.5, -0.5, -0.5];
    let p001 = [-0.5, -0.5, 0.5];
    let p010 = [-0.5, 0.5, -0.5];
    let p011 = [-0.5, 0.5, 0.5];
    let p100 = [0.5, -0.5, -0.5];
    let p101 = [0.5, -0.5, 0.5];
    let p110 = [0.5, 0.5, -0.5];
    let p111 = [0.5, 0.5, 0.5];

    push_face(&mut vertices, p001, p101, p111, p011, [0.0, 0.0, 1.0]);
    push_face(&mut vertices, p100, p110, p111, p101, [1.0, 0.0, 0.0]);
    push_face(&mut vertices, p000, p001, p011, p010, [-1.0, 0.0, 0.0]);
    push_face(&mut vertices, p010, p011, p111, p110, [0.0, 1.0, 0.0]);
    push_face(&mut vertices, p000, p100, p101, p001, [0.0, -1.0, 0.0]);
    push_face(&mut vertices, p000, p010, p110, p100, [0.0, 0.0, -1.0]);

    vertices
}

