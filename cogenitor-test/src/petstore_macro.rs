#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

cogenitor::generate_api!("test-data/petstore.yaml");

#[cfg(test)]
mod tests {
    use super::generated_api;
    use serde_json::json;

    #[test]
    pub fn test_generated_api() {
        // first, try to construct a Pet instance
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

        // serialize that instance to a JSON value and compare
        // with an expected JSON representation
        let value = serde_json::to_value(&pet).unwrap();
        let expected_value = json!({
            "id": 1,
            "name": "Doggy",
            "category": {
                "id": 1000,
                "name": "Dogs"
            },
            "status": "placed",
            "photoUrls": [],
            "tags": [],
        });
        assert_eq!(expected_value, value);

        // try round-trip: deserialize the serialized value back into a Pet
        let other_pet = serde_json::from_value::<generated_api::Pet>(value).unwrap();
        assert_eq!(pet, other_pet);
    }
}
