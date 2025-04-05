# structr

A command-line tool for generating Rust structs from JSON data. Easily convert JSON samples into ready-to-use Rust struct definitions with serde support and framework integrations.

[![Crates.io](https://img.shields.io/crates/v/structr.svg)](https://crates.io/crates/structr)
[![License](https://img.shields.io/crates/l/structr.svg)](https://github.com/yourusername/structr/blob/main/LICENSE)

## Features

- Convert JSON data to Rust struct definitions
- Support for nested objects and arrays
- Auto-generate serde annotations
- Process multiple JSON samples to improve type accuracy
- Automatically handle optional fields
- Read from files or stdin
- Detect and generate enums for string fields with limited values
- Flatten nested objects with `serde(flatten)`
- Generate code compatible with popular frameworks:
  - Actix Web
  - Axum
  - Rocket
- Generate GraphQL schema and resolvers

## Installation

```bash
cargo install structr
```

Or build from source:

```bash
git clone https://github.com/yourusername/structr.git
cd structr
cargo build --release
```

## Usage

### Basic Usage

```bash
# Generate structs from a JSON file
structr --input data.json --output models.rs

# Specify a custom root struct name
structr --input data.json --name ApiResponse
```

### Process Multiple Files

Combine multiple samples to get more accurate type definitions:

```bash
structr --input sample1.json --input sample2.json
```

### Using STDIN

```bash
# Pipe JSON directly to structr
cat data.json | structr

# Or explicitly use stdin flag
curl https://api.example.com/data | structr --stdin
```

### Optional Fields

Make all fields optional with `Option<T>`:

```bash
structr --input data.json --strict-option
```

### Flattening Nested Objects

Use `#[serde(flatten)]` for nested objects:

```bash
structr --input data.json --flatten
```

### Framework Integration

Generate code with framework-specific annotations:

```bash
# Actix Web
structr --input data.json --actix

# Axum
structr --input data.json --axum

# Rocket
structr --input data.json --rocket
```

### GraphQL Support

Generate code with GraphQL support:

```bash
# Generate GraphQL-compatible structs with async-graphql
structr --input data.json --graphql

# Generate GraphQL-compatible structs with juniper
structr --input data.json --graphql --graphql-lib juniper

# Generate a GraphQL schema file
structr --input data.json --graphql --graphql-schema

# Specify output directory for schema
structr --input data.json --graphql --graphql-schema --schema-dir ./schemas
```

## Examples

### Input JSON

```json
{
  "id": 123,
  "name": "Product",
  "price": 29.99,
  "tags": ["electronics", "gadget"],
  "dimensions": {
    "width": 10,
    "height": 5,
    "unit": "cm"
  },
  "in_stock": true
}
```

### Generated Rust Code

```rust
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Dimensions {
    pub width: i64,
    pub height: i64,
    pub unit: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RootStruct {
    pub id: i64,
    pub name: String,
    pub price: f64,
    pub tags: Vec<String>,
    pub dimensions: Dimensions,
    pub in_stock: bool,
}
```

### With Actix Web Support

```rust
use serde::{Serialize, Deserialize};
use actix_web::web;

#[derive(Debug, Serialize, Deserialize, actix_web::web::Query, actix_web::web::Path, actix_web::web::Json)]
pub struct Dimensions {
    pub width: i64,
    pub height: i64,
    pub unit: String,
}

impl Dimensions {
    pub fn into_json(self) -> actix_web::web::Json<Self> {
        actix_web::web::Json(self)
    }
}

#[derive(Debug, Serialize, Deserialize, actix_web::web::Query, actix_web::web::Path, actix_web::web::Json)]
pub struct RootStruct {
    pub id: i64,
    pub name: String,
    pub price: f64,
    pub tags: Vec<String>,
    pub dimensions: Dimensions,
    pub in_stock: bool,
}

impl RootStruct {
    pub fn into_json(self) -> actix_web::web::Json<Self> {
        actix_web::web::Json(self)
    }
}
```

### With GraphQL Support

```rust
use serde::{Serialize, Deserialize};
use async_graphql::{SimpleObject, Object, Context};

#[derive(Debug, Serialize, Deserialize, SimpleObject)]
#[graphql(complex)]
pub struct Dimensions {
    pub width: i64,
    pub height: i64,
    pub unit: String,
}

#[derive(Debug, Serialize, Deserialize, SimpleObject)]
#[graphql(complex)]
pub struct RootStruct {
    pub id: i64,
    pub name: String,
    pub price: f64,
    pub tags: Vec<String>,
    pub dimensions: Dimensions,
    pub in_stock: bool,
}

#[derive(Default)]
pub struct Query;

#[Object]
impl Query {
    async fn get_dimensions(&self, ctx: &Context<'_>) -> Result<Dimensions, async_graphql::Error> {
        // Add your resolver implementation here
        unimplemented!()
    }

    async fn get_root_struct(&self, ctx: &Context<'_>) -> Result<RootStruct, async_graphql::Error> {
        // Add your resolver implementation here
        unimplemented!()
    }
}

#[derive(Default)]
pub struct Mutation;

#[Object]
impl Mutation {
    async fn create_dimensions(&self, ctx: &Context<'_>, input: Dimensions) -> Result<Dimensions, async_graphql::Error> {
        // Add your resolver implementation here
        unimplemented!()
    }

    async fn update_dimensions(&self, ctx: &Context<'_>, id: String, input: Dimensions) -> Result<Dimensions, async_graphql::Error> {
        // Add your resolver implementation here
        unimplemented!()
    }

    async fn delete_dimensions(&self, ctx: &Context<'_>, id: String) -> Result<bool, async_graphql::Error> {
        // Add your resolver implementation here
        unimplemented!()
    }

    // ... more resolver methods for RootStruct
}

pub type RootStructSchema = async_graphql::Schema<Query, Mutation, async_graphql::EmptySubscription>;

pub fn create_root_struct_schema() -> RootStructSchema {
    RootStructSchema::build(Query::default(), Mutation::default(), async_graphql::EmptySubscription::default()).finish()
}
```

## Advanced Usage

### Enum Detection

When providing multiple JSON samples with a field that has a limited set of string values, structr will attempt to generate an enum:

```json
// sample1.json
{"status": "pending", "id": 1}

// sample2.json
{"status": "completed", "id": 2}

// sample3.json
{"status": "failed", "id": 3}
```

Will generate:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub enum StatusEnum {
    Pending,
    Completed,
    Failed,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RootStruct {
    pub status: StatusEnum,
    pub id: i64,
}
```

### Combined Options Example

```bash
# Use all features
structr --input data.json --strict-option --flatten --name ApiResponse --actix --graphql --graphql-schema
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the LICENSE file for details.
