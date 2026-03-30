

/*
trait Scene {
    update(&self);
    draw(&self, |batcher| {});
}
 */


/*

canvas
    .init()
    .shading(Phong)
    .camera.
    .update_frequency(u32)
    .update(|| {
    })
    .draw(|batcher| {
        batcher.addCircle();
        ....
        batcher.addCube();
    })
    .start();


 */


/*
fn pack_instances_for_mesh(instances: &[InstanceRecord], mesh_index: u32) -> Vec<f32> {
        let mut packed = Vec::with_capacity(instances.len() * 8);

        for instance in instances.iter().filter(|instance| instance.mesh_index == mesh_index) {
            packed.extend_from_slice(&[
                instance.position[0],
                instance.position[1],
                instance.position[2],
                instance.rotation[0],
                instance.rotation[1],
                instance.rotation[2],
                instance.rotation[3],
                instance.mesh_index as f32,
            ]);
        }

        packed
    }
 */
use std::collections::HashMap;
use web_sys::{WebGl2RenderingContext, WebGlProgram};
use crate::MeshBatch;
use crate::primitive::cube;

#[allow(dead_code)]
pub struct Batcher {
    gl: WebGl2RenderingContext,
    batches: HashMap<u32, MeshBatch>,
    pending_instances: HashMap<u32, Vec<f32>>,
}

#[allow(dead_code)]
impl Batcher {

    fn pack_instance(position: [f32; 3], rotation: [f32; 4], mesh_index: u32) -> [f32; 8] {
        [
            position[0],
            position[1],
            position[2],
            rotation[0],
            rotation[1],
            rotation[2],
            rotation[3],
            mesh_index as f32,
        ]
    }

    pub fn new(gl: &WebGl2RenderingContext, web_gl_program: &WebGlProgram) -> Self {
        let mut map = HashMap::new();

        map.insert(0_u32, MeshBatch::new(gl, web_gl_program, &cube(), &[]).unwrap());

        Self {
            gl: gl.clone(),
            batches: map,
            pending_instances: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.pending_instances.clear();
    }

    pub fn add_cube(&mut self, position: [f32; 3], rotation: [f32; 4]) {
        self.pending_instances
            .entry(0)
            .or_default()
            .extend_from_slice(&Self::pack_instance(position, rotation, 0));
    }

    pub fn flush(&self) {
        for (mesh_index, batch) in self.batches.iter() {
            let instance_data = self
                .pending_instances
                .get(mesh_index)
                .map_or(&[][..], |instances| instances.as_slice());

            batch.upload_instances(&self.gl, instance_data);
        }
    }

    pub fn draw(&self) {
        for batch in self.batches.values() {
            self.gl.bind_vertex_array(Some(&batch._vao));
            self.gl.draw_arrays_instanced(
                WebGl2RenderingContext::TRIANGLES,
                0,
                batch.vertex_count,
                batch.instance_count.get(),
            );
        }
    }
}