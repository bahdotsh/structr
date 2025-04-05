# structr

A command-line tool for generating Rust structs from JSON data. Easily convert JSON samples into ready-to-use Rust struct definitions with serde support.

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
structr --input data.json --strict-option --flatten --name ApiResponse
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
