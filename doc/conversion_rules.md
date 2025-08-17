# Conversion Rules
The generator applies a well defined set of rules to convert [OpenAPI](https://spec.openapis.org) documents to Rust code.

## Principles

Code generation follows the following principles:
* Every valid OpenAPI document will be tranformed into valid Rust code. This means that
  the goal is to support every construct allowed by the spec.
* Generated code should be ergonomic. This may be in conflict with the first rule, however
  the generator may apply simplifications where it deems necessary
* Generated code must be usable without modification.
* Generated code should be stable with regards to small changes in the spec document. This
  means that local changes to the spec should only result in local changes to the generated code. Violation to this rule causes code relying on generated code to break.
* The generator aims to support all major OpenAPI versions (at the time of writing this would be 2.0, 3.0 and 3.1)

## General notes about code generation

Code will be generated in a module whose name is configurable. Only generated code will
live inside this module. This helps prevent interference with surrounding user Rust code.

## Mapping OpenAPI's JSON Schema flavor to Rust

Where possible, the generator will use Rust's built-in types. The types chosen depend
on the the `type` and the `format` set in the schema definition (see also the respective section in the [OpenAPI 3.0 spec](https://spec.openapis.org/oas/v3.0.4.html#x4-4-1-data-type-format))

When new types are generated, their names are derived from the names in the `schemas` object
(`#/components/schemas/`). So a an `object` type in `#/components/schemas/Foo` will be mapped as `struct Foo`.

TODO: Mapping inline schemata


### Mapping `string`

`string` types are mapped to Rust's `String` type.

TODO: we will map function parameters to `&str` in the future.

TODO: Map `string` types with `enum` to actual Rust enums

TODO: Other string formats from https://spec.openapis.org/oas/v3.0.4.html#x4-4-1-data-type-format


### Mapping `number` (and it's `integer`)

| `type`   | `format`  | Rust type(s)     |
|----------|-----------|------------------|
| `number` | `int32`   | `i32`            |
| `number` | `int64`   | `i64`            |
| `number` | `float`   | `f32`            |
| `number` | `double`  | `f64`            |
| `number` | any other | `f64`            |


### Mapping `boolean`

Booleans always map to `bool`.

### Mapping `array`

In general, arrays are mapped to `Vec<T>`, where `T` is the type mapped from the schema in the `items`
property.


### Mapping `object`

Schemas with type `object` are mapped to generated Rust `struct`s.

TODO: Support `allOf` / `anyOf`

TODO: Support `oneOf` polymorphism by generating Rust enums

TODO: Support `additionalProperties` via `HashMap<String,V>`


### Mapping `null`

TODO: Support mapping `null`, even though this does not make much sense. For Rust, the unit type
`()` seems appropriate.


## Client code generation from OpenAPI operations

The generator will produce a struct called `Client`, for which it produces an `impl` block. The block will contains methods for each operation (so each HTTP verb like `get`, `put` etc. will produce its own method). The method names derive from the path and the HTTP verb, so the endpoint for `GET /foo/bar` will become `pub fn foo_bar_get(&self)`.

An OpenAPI operation consits of a number of key components that each influence the way a method is generated.

Every method has `Result<T,E>` as it's return type. The actual types used for `T` and `E` depend on the responses object.

TODO: Async methods

### Parameters

Parameters are mapped directly to operation method parameters. Rust method parameter types derive by the usual rules for type mapping (see above).

TODO: instead of useing `String`, use `&str` for parameters.


### TODO: RequestBody

### TODO: Responses

Broadly speaking, we distinguish between _known_ responses (that is, responses explicitely declared in the OpenAPI file) and _unknown_ responses (responses that the server yields, but are not defined in the API). Unknown responses are generally treated as an error, regardless of their response code (when an endpoint declared to respond with 200 returns 201, this is an error in terms of the spec).

In terms of the responses declared in the OpenAPI file we devide those into two categories: Success responses (2xx) and Non-Success responses (1xx, 3xx-5xx, or any other non-2xx code).

The responses of an OpenAPI operation may link to a named response in the `#/components/responses` path. The names there may be used to name generated types.

Each `Client` method returns a `Result<T,E>`, where `T` represents the success response category (2xx), and `E` non-success responses (non 2xx HTTP statuses as well as other errors alike).

TODO: what to do with 'default' responses?


#### TODO: Success Responses

If there is no success response defined (no 2xx code present), `T` maps to the Rust unit type `()`.

If there is exactly one success response defined, `T` maps to the type that is mapped for the media type(s) in `content` (see section below)

If there are multiple success responses defined, `T` maps to an enum that is generated for this purpose. The enum carries the name  {operationFragment}`Success` (`GET /foo/bar` will become `FooBarSuccess`). For each success response, a variant for this response will be generated. The variants in the enum are called by the respective name of the HTTP code, so 200 becomes `Ok200`, 201 becomes `Created201`, etc.


#### TODO: Non-Success responses

Non-Success responses as well as other potential errors that can happen when calling a HTTP endpoint must be represented by the error type `E` in a method's  `Result<T,E>` return type.

Therefore, E must be able to represent:
* Defined error responses, distinguished by their HTTP status
* Non-HTTP errors such as network or general I/O errors
* Undefined responses, success or error alike

For this reason, `E` will always be a generated Rust enum.

The enum is generated as follows:
* The name is composed of {operationFragment}`Error`. So for `PUT /pet`, the generated enum will be called `PutPetError`.
* The variants are defined like this:
  - Declared error codes, such as HTTP 400, are called after their {statusFragment}, so HTTP 400 becomes `NotFound400`. For each declared HTTP error (4xx or 5xx ranges), such a variant is generated. The variants are generated as tuple variants, whose single member type is the type yielded by mapping the media type of that response (see section below)
  - For undeclared HTTP responses, a variant called `UnknownResponse` is generated. The variant is generated as a tuple variant whoose type is `http::Response` from the `http` crate.
  - For all other errors, a tuple variant `OtherError` is generated. It's contained type is `anyhow::Error` from the `anyhow` crate.


#### Media type mappings

A response defined in OpenAPI may define schemata for one or more media types. Consider the following fragment:

```yaml
      responses:
        "200":
          description: Successful operation
          content:
            application/json:
              schema:
                $ref: "#/components/schemas/Pet"
            application/xml:
              schema:
                $ref: "#/components/schemas/Pet"
```

Note that the example above is taken from the petstore example for OpenAPI which defines the same schema for the XML and JSON media types, but this may not be the case for other OpenAPI for other OpenAPI definitions.

The type mapped to a response follows the following rules:
* If a response does not have a content field or the content map is empty, the mapped type is the unit type `()`
* If a response contains only one media type, the type of the response is the type mapped for this media type.
* If there is more than one media type object present, an enum is generated to distinguish between media types. The enum is generated according to the following rules:
  - The name of the enum is {operationFragment}{statusFragment}`Content`. For `PUT /pet`, {operationFragment} is `PetPut`. For the HTTP 200 example above, the {statusFragment} is `Ok200`. So the resulting name of the enum is `PetPutOk200Content`.
  - For each media type, an enum variant is introduced. Media types are translated into Rust enum variant names by using the type and subtype and uppercasing each of their first characters. Then both are joint into a string (ignoring the '/' separator). Media type wildcard characters (`*`) present in the type or subtype are replaced by the string `Any`. So a media type `application/json` becomes an enum variant called `ApplicationJson`. A media type wildcard `text/*` will become `TextAny`.

With the rules above, the example shown will generate the following enum:
```rust
enum PetPutOk200Content {
    ApplicationJson(Pet),
    ApplicationXml(Pet),
}
```
