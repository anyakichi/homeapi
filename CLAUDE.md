# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository Overview

This is a Rust-based GraphQL API service (homeapi) designed to run on AWS Lambda with DynamoDB as the data store. The project supports both AWS Lambda deployment and local development server modes.

## Common Development Commands

### Building the Project

```bash
# Build the project
cargo build

# Build for release
cargo build --release

# Build the Lambda handler binary
cargo build --release --bin bootstrap

# Build the local development server
cargo build --release --bin homeapi
```

### Running the Development Server

```bash
# Set the required environment variable
export TABLE_NAME=your-dynamodb-table-name

# Run the local development server (port 8080)
cargo run --bin homeapi

# Access GraphQL playground at http://localhost:8080/
```

### Docker Build

```bash
# Build the Docker image for Lambda deployment
docker build -t homeapi .
```

### Checking Code

```bash
# Format code
cargo fmt

# Run clippy for linting
cargo clippy

# Check if the project compiles
cargo check
```

## Architecture Overview

### Project Structure

- `src/lib.rs` - Library root that exports all modules
- `src/dynamodb.rs` - DynamoDB client wrapper implementation
- `src/graphql.rs` - GraphQL schema, resolvers, and type definitions
- `src/models.rs` - Data models and structures
- `src/bin/bootstrap.rs` - AWS Lambda handler entry point
- `src/bin/homeapi.rs` - Local development server entry point

### Key Design Patterns

1. **Dual Deployment Mode**: The project separates the Lambda handler (`bootstrap`) from the local development server (`homeapi`), both using the same core GraphQL schema.

2. **GraphQL Schema**: Uses async-graphql with warp integration. The schema is initialized once using `once_cell::sync::Lazy` for performance.

3. **DynamoDB Integration**: All DynamoDB operations are abstracted through a `Client` wrapper in `src/dynamodb.rs`, which handles serialization/deserialization using `serde_dynamodb`.

4. **Environment Configuration**: Uses environment variables for configuration:
   - `TABLE_NAME` - Required for DynamoDB table name
   - AWS credentials and region are handled by standard AWS SDK environment variables

### Development Considerations

- The project uses Rust edition 2024
- TLS is handled via rustls (not OpenSSL) for better portability
- The Docker build uses a custom Lambda Rust builder image
- No test suite currently exists - consider adding tests when implementing new features
