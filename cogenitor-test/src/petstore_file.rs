include!(concat!(env!("OUT_DIR"), "/petstore.rs"));

#[test]
pub fn test_pet_present() {
    #[allow(unused)]
    use generated_api::Pet;

    // this test does not fail; if it compiles it means that the Pet type is available
}
