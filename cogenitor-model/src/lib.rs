//mod client_adapter;

/// model code for a REST client.
/// This is for experimentation, so actual client code can be prototyped here and then
/// used to model generated code after it.
#[derive(
    :: std :: fmt :: Debug,
    :: serde :: Serialize,
    :: serde ::
Deserialize,
    :: core :: cmp :: PartialEq,
)]
pub struct Order {
    pub petId: i64,
    pub id: i64,
    pub shipDate: String,
    pub status: String,
    pub quantity: i32,
    pub complete: bool,
}
#[derive(
    :: std :: fmt :: Debug,
    :: serde :: Serialize,
    :: serde ::
Deserialize,
    :: core :: cmp :: PartialEq,
)]
pub struct Category {
    pub name: String,
    pub id: i64,
}
#[derive(
    :: std :: fmt :: Debug,
    :: serde :: Serialize,
    :: serde ::
Deserialize,
    :: core :: cmp :: PartialEq,
)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub firstName: String,
    pub phone: String,
    pub username: String,
    pub password: String,
    pub userStatus: i32,
    pub lastName: String,
}
#[derive(
    :: std :: fmt :: Debug,
    :: serde :: Serialize,
    :: serde ::
Deserialize,
    :: core :: cmp :: PartialEq,
)]
pub struct Tag {
    pub name: String,
    pub id: i64,
}
#[derive(
    :: std :: fmt :: Debug,
    :: serde :: Serialize,
    :: serde ::
Deserialize,
    :: core :: cmp :: PartialEq,
)]
pub struct Pet {
    pub photoUrls: Vec<String>,
    pub category: Category,
    pub tags: Vec<Tag>,
    pub name: String,
    pub id: i64,
    pub status: String,
}
#[derive(
    :: std :: fmt :: Debug,
    :: serde :: Serialize,
    :: serde ::
Deserialize,
    :: core :: cmp :: PartialEq,
)]
pub struct ApiResponse {
    pub code: i32,
    pub message: String,
    pub type_: String,
}

enum ApiError {
    /// a response that was not defined in the spec
    UndefinedResponse,
}
pub struct UndefindedResponse {}
#[derive(Debug)]
pub struct Client {}

enum PetPutOk200Content {
    ApplicationJson(Pet),
    ApplicationXml(Pet),
}

enum PetPutError {
    Status400,
    Status404,
    Status422,
    UndefinedError,
    UndefinedResponse,
}

impl Client {
    pub fn pet_put(self: &Self) -> Result<PetPutOk200Content, PetPutError> {
        todo!()
    }
    pub fn pet_post(self: &Self) -> Result<(), ()> {
        todo!()
    }
    pub fn pet_findbystatus_get(self: &Self, status: String) -> Result<(), ()> {
        todo!()
    }
    pub fn pet_findbytags_get(self: &Self, tags: Vec<String>) -> Result<(), ()> {
        todo!()
    }
    pub fn pet_petid_get(self: &Self, petId: i64) -> Result<(), ()> {
        todo!()
    }
    pub fn pet_petid_post(self: &Self, petId: i64, name: String, status: String) -> Result<(), ()> {
        todo!()
    }
    pub fn pet_petid_delete(self: &Self, api_key: String, petId: i64) -> Result<(), ()> {
        todo!()
    }
    pub fn pet_petid_uploadimage_post(
        self: &Self,
        petId: i64,
        additionalMetadata: String,
    ) -> Result<(), ()> {
        todo!()
    }
}
