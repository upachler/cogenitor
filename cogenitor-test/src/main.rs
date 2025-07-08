cogenitor::generate_api!("test-data/petstore.yaml");

fn main() {
    let pet = generated_api::Pet {
        id: 1,
        name: "Doggy".to_string(),
        category: generated_api::Category {
            id: 1000,
            name: "Dogs".to_string(),
        },
        status: "placed".to_string(),
        photoUrls: vec![],
        tags: vec![],
    };
    println!("Hello, world!");
}
