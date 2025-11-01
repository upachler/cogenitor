//mod client_adapter;

/// model code for a REST client.
/// This is for experimentation, so actual client code can be prototyped here and then
/// used to model generated code after it.
pub mod generated_api {
    #![allow(unused_imports)]
    #![allow(dead_code)]
    #![allow(unused_variables)]
    #![allow(non_snake_case)]
    #![allow(non_camel_case_types)]
    use std::path::Path;
    #[derive(
        :: std :: fmt :: Debug,
        :: serde :: Serialize,
        :: serde :: Deserialize,
        :: core :: cmp :: PartialEq,
    )]
    pub struct Order {
        pub status: String,
        pub complete: bool,
        pub quantity: i32,
        pub id: i64,
        pub petId: i64,
        pub shipDate: String,
    }
    #[derive(
        :: std :: fmt :: Debug,
        :: serde :: Serialize,
        :: serde :: Deserialize,
        :: core :: cmp :: PartialEq,
    )]
    pub struct Category {
        pub id: i64,
        pub name: String,
    }
    #[derive(
        :: std :: fmt :: Debug,
        :: serde :: Serialize,
        :: serde :: Deserialize,
        :: core :: cmp :: PartialEq,
    )]
    pub struct User {
        pub id: i64,
        pub lastName: String,
        pub firstName: String,
        pub email: String,
        pub password: String,
        pub userStatus: i32,
        pub username: String,
        pub phone: String,
    }
    #[derive(
        :: std :: fmt :: Debug,
        :: serde :: Serialize,
        :: serde :: Deserialize,
        :: core :: cmp :: PartialEq,
    )]
    pub struct Tag {
        pub name: String,
        pub id: i64,
    }
    #[derive(
        :: std :: fmt :: Debug,
        :: serde :: Serialize,
        :: serde :: Deserialize,
        :: core :: cmp :: PartialEq,
    )]
    pub struct Pet {
        pub category: Category,
        pub status: String,
        pub id: i64,
        pub tags: Vec<Tag>,
        pub name: String,
        pub photoUrls: Vec<String>,
    }
    #[derive(
        :: std :: fmt :: Debug,
        :: serde :: Serialize,
        :: serde :: Deserialize,
        :: core :: cmp :: PartialEq,
    )]
    pub struct ApiResponse {
        pub message: String,
        pub code: i32,
        pub type_: String,
    }
    #[derive(Debug)]
    pub struct Client {}
    pub enum PetPutOk200 {
        ApplicationJson(Pet),
        ApplicationXml(Pet),
    }
    pub enum PetPutError {
        BadRequest400(()),
        NotFound404(()),
        UnprocessableEntity422(()),
        UnknownResponse(::http::Response<::std::vec::Vec<u8>>),
        OtherError(::std::boxed::Box<dyn ::std::error::Error>),
    }
    pub enum PetPutContent {
        ApplicationJson(Pet),
        ApplicationXml(Pet),
        ApplicationXwwwformurlencoded(Pet),
    }
    pub enum PetPostOk200 {
        ApplicationJson(Pet),
        ApplicationXml(Pet),
    }
    pub enum PetPostError {
        BadRequest400(()),
        UnprocessableEntity422(()),
        UnknownResponse(::http::Response<::std::vec::Vec<u8>>),
        OtherError(::std::boxed::Box<dyn ::std::error::Error>),
    }
    pub enum PetPostContent {
        ApplicationJson(Pet),
        ApplicationXml(Pet),
        ApplicationXwwwformurlencoded(Pet),
    }
    pub enum PetFindByStatusGetOk200 {
        ApplicationXml(Vec<Pet>),
        ApplicationJson(Vec<Pet>),
    }
    pub enum PetFindByStatusGetError {
        BadRequest400(()),
        UnknownResponse(::http::Response<::std::vec::Vec<u8>>),
        OtherError(::std::boxed::Box<dyn ::std::error::Error>),
    }
    pub enum PetFindByTagsGetOk200 {
        ApplicationXml(Vec<Pet>),
        ApplicationJson(Vec<Pet>),
    }
    pub enum PetFindByTagsGetError {
        BadRequest400(()),
        UnknownResponse(::http::Response<::std::vec::Vec<u8>>),
        OtherError(::std::boxed::Box<dyn ::std::error::Error>),
    }
    pub enum PetPetIdGetOk200 {
        ApplicationJson(Pet),
        ApplicationXml(Pet),
    }
    pub enum PetPetIdGetError {
        BadRequest400(()),
        NotFound404(()),
        UnknownResponse(::http::Response<::std::vec::Vec<u8>>),
        OtherError(::std::boxed::Box<dyn ::std::error::Error>),
    }
    pub enum PetPetIdPostOk200 {
        ApplicationJson(Pet),
        ApplicationXml(Pet),
    }
    pub enum PetPetIdPostError {
        BadRequest400(()),
        UnknownResponse(::http::Response<::std::vec::Vec<u8>>),
        OtherError(::std::boxed::Box<dyn ::std::error::Error>),
    }
    pub enum PetPetIdDeleteError {
        BadRequest400(()),
        UnknownResponse(::http::Response<::std::vec::Vec<u8>>),
        OtherError(::std::boxed::Box<dyn ::std::error::Error>),
    }
    pub enum PetPetIdUploadImagePostError {
        BadRequest400(()),
        NotFound404(()),
        UnknownResponse(::http::Response<::std::vec::Vec<u8>>),
        OtherError(::std::boxed::Box<dyn ::std::error::Error>),
    }
    #[derive(
        :: std :: fmt :: Debug,
        :: serde :: Serialize,
        :: serde :: Deserialize,
        :: core :: cmp :: PartialEq,
    )]
    pub struct StoreInventoryGetOk200 {}
    pub enum StoreInventoryGetError {
        UnknownResponse(::http::Response<::std::vec::Vec<u8>>),
        OtherError(::std::boxed::Box<dyn ::std::error::Error>),
    }
    pub enum StoreOrderPostError {
        BadRequest400(()),
        UnprocessableEntity422(()),
        UnknownResponse(::http::Response<::std::vec::Vec<u8>>),
        OtherError(::std::boxed::Box<dyn ::std::error::Error>),
    }
    pub enum StoreOrderPostContent {
        ApplicationXml(Order),
        ApplicationXwwwformurlencoded(Order),
        ApplicationJson(Order),
    }
    pub enum StoreOrderOrderIdGetOk200 {
        ApplicationJson(Order),
        ApplicationXml(Order),
    }
    pub enum StoreOrderOrderIdGetError {
        BadRequest400(()),
        NotFound404(()),
        UnknownResponse(::http::Response<::std::vec::Vec<u8>>),
        OtherError(::std::boxed::Box<dyn ::std::error::Error>),
    }
    pub enum StoreOrderOrderIdDeleteError {
        BadRequest400(()),
        NotFound404(()),
        UnknownResponse(::http::Response<::std::vec::Vec<u8>>),
        OtherError(::std::boxed::Box<dyn ::std::error::Error>),
    }
    pub enum UserPostOk200 {
        ApplicationXml(User),
        ApplicationJson(User),
    }
    pub enum UserPostError {
        UnknownResponse(::http::Response<::std::vec::Vec<u8>>),
        OtherError(::std::boxed::Box<dyn ::std::error::Error>),
    }
    pub enum UserPostContent {
        ApplicationXwwwformurlencoded(User),
        ApplicationJson(User),
        ApplicationXml(User),
    }
    pub enum UserCreateWithListPostOk200 {
        ApplicationJson(User),
        ApplicationXml(User),
    }
    pub enum UserCreateWithListPostError {
        UnknownResponse(::http::Response<::std::vec::Vec<u8>>),
        OtherError(::std::boxed::Box<dyn ::std::error::Error>),
    }
    pub enum UserLoginGetOk200 {
        ApplicationXml(String),
        ApplicationJson(String),
    }
    pub enum UserLoginGetError {
        BadRequest400(()),
        UnknownResponse(::http::Response<::std::vec::Vec<u8>>),
        OtherError(::std::boxed::Box<dyn ::std::error::Error>),
    }
    pub enum UserLogoutGetError {
        UnknownResponse(::http::Response<::std::vec::Vec<u8>>),
        OtherError(::std::boxed::Box<dyn ::std::error::Error>),
    }
    pub enum UserUsernameGetOk200 {
        ApplicationJson(User),
        ApplicationXml(User),
    }
    pub enum UserUsernameGetError {
        BadRequest400(()),
        NotFound404(()),
        UnknownResponse(::http::Response<::std::vec::Vec<u8>>),
        OtherError(::std::boxed::Box<dyn ::std::error::Error>),
    }
    pub enum UserUsernamePutError {
        BadRequest400(()),
        NotFound404(()),
        UnknownResponse(::http::Response<::std::vec::Vec<u8>>),
        OtherError(::std::boxed::Box<dyn ::std::error::Error>),
    }
    pub enum UserUsernamePutContent {
        ApplicationJson(User),
        ApplicationXwwwformurlencoded(User),
        ApplicationXml(User),
    }
    pub enum UserUsernameDeleteError {
        BadRequest400(()),
        NotFound404(()),
        UnknownResponse(::http::Response<::std::vec::Vec<u8>>),
        OtherError(::std::boxed::Box<dyn ::std::error::Error>),
    }
    impl Client {
        pub fn pet_put(self: &Self, body: PetPutContent) -> Result<PetPutOk200, PetPutError> {
            todo!()
        }
        pub fn pet_post(self: &Self, body: PetPostContent) -> Result<PetPostOk200, PetPostError> {
            todo!()
        }
        pub fn pet_findbystatus_get(
            self: &Self,
            status: String,
        ) -> Result<PetFindByStatusGetOk200, PetFindByStatusGetError> {
            todo!()
        }
        pub fn pet_findbytags_get(
            self: &Self,
            tags: Vec<String>,
        ) -> Result<PetFindByTagsGetOk200, PetFindByTagsGetError> {
            todo!()
        }
        pub fn pet_petid_get(
            self: &Self,
            petId: i64,
        ) -> Result<PetPetIdGetOk200, PetPetIdGetError> {
            todo!()
        }
        pub fn pet_petid_post(
            self: &Self,
            petId: i64,
            name: String,
            status: String,
        ) -> Result<PetPetIdPostOk200, PetPetIdPostError> {
            todo!()
        }
        pub fn pet_petid_delete(
            self: &Self,
            api_key: String,
            petId: i64,
        ) -> Result<(), PetPetIdDeleteError> {
            todo!()
        }
        pub fn pet_petid_uploadimage_post(
            self: &Self,
            petId: i64,
            additionalMetadata: String,
            body: String,
        ) -> Result<ApiResponse, PetPetIdUploadImagePostError> {
            todo!()
        }
        pub fn store_inventory_get(
            self: &Self,
        ) -> Result<StoreInventoryGetOk200, StoreInventoryGetError> {
            todo!()
        }
        pub fn store_order_post(
            self: &Self,
            body: StoreOrderPostContent,
        ) -> Result<Order, StoreOrderPostError> {
            todo!()
        }
        pub fn store_order_orderid_get(
            self: &Self,
            orderId: i64,
        ) -> Result<StoreOrderOrderIdGetOk200, StoreOrderOrderIdGetError> {
            todo!()
        }
        pub fn store_order_orderid_delete(
            self: &Self,
            orderId: i64,
        ) -> Result<(), StoreOrderOrderIdDeleteError> {
            todo!()
        }
        pub fn user_post(
            self: &Self,
            body: UserPostContent,
        ) -> Result<UserPostOk200, UserPostError> {
            todo!()
        }
        pub fn user_createwithlist_post(
            self: &Self,
            body: Vec<User>,
        ) -> Result<UserCreateWithListPostOk200, UserCreateWithListPostError> {
            todo!()
        }
        pub fn user_login_get(
            self: &Self,
            username: String,
            password: String,
        ) -> Result<UserLoginGetOk200, UserLoginGetError> {
            todo!()
        }
        pub fn user_logout_get(self: &Self) -> Result<(), UserLogoutGetError> {
            todo!()
        }
        pub fn user_username_get(
            self: &Self,
            username: String,
        ) -> Result<UserUsernameGetOk200, UserUsernameGetError> {
            todo!()
        }
        pub fn user_username_put(
            self: &Self,
            username: String,
            body: UserUsernamePutContent,
        ) -> Result<(), UserUsernamePutError> {
            todo!()
        }
        pub fn user_username_delete(
            self: &Self,
            username: String,
        ) -> Result<(), UserUsernameDeleteError> {
            todo!()
        }
    }
}
