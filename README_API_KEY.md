# API Key Authentication Implementation

## Overview

This implementation adds API key authentication alongside the existing Google OAuth authentication. The API keys use an efficient DynamoDB schema design for fast lookups.

## DynamoDB Schema

### Primary Table Structure

- **PK**: SHA256 hash of the API key
- **SK**: "APIKEY" (constant)
- **Attributes**: user_email, name, created_at, last_used_at, expires_at

### Global Secondary Index (GSI)

- **Index Name**: `user_email-index`
- **Partition Key**: `user_email`

This design allows:

- O(1) lookup for API key verification (direct PK lookup)
- Efficient query for all API keys belonging to a user (GSI query)

## API Key Format

API keys follow the format: `ha_` + UUID v4 (without hyphens)

- Example: `ha_550e8400e29b41d4a716446655440000`
- Total length: 35 characters

## GraphQL API

### Mutations

```graphql
# Create a new API key
mutation {
  createApiKey(
    name: "My API Key"
    expiresAt: "2025-12-31T23:59:59Z" # Optional
  ) {
    apiKey {
      id
      name
      createdAt
      expiresAt
    }
    key # The actual API key - only shown on creation
  }
}

# Delete an API key
mutation {
  deleteApiKey(id: "ApiKey:hash:APIKEY") {
    success
  }
}
```

### Queries

```graphql
# List all API keys for the authenticated user
query {
  apiKeys {
    id
    name
    createdAt
    lastUsedAt
    expiresAt
  }
}
```

## Usage

Include the API key in the Authorization header:

```bash
curl -H "Authorization: Bearer ha_xxxxx..." \
     -H "Content-Type: application/json" \
     -d '{"query": "{ devices { edges { node { place } } } }"}' \
     http://localhost:8080/graphql
```

## Security Features

1. **SHA256 Hashing**: API keys are never stored in plain text
2. **Expiration Support**: Optional expiration dates for API keys
3. **Last Used Tracking**: Automatically updates last_used_at timestamp
4. **User Isolation**: Users can only see/manage their own API keys

## Setting up the GSI

To create the required GSI in DynamoDB, use the AWS CLI:

```bash
aws dynamodb update-table \
  --table-name YourTableName \
  --attribute-definitions \
    AttributeName=user_email,AttributeType=S \
  --global-secondary-index-updates \
    '[{
      "Create": {
        "IndexName": "user_email-index",
        "Keys": [
          {"AttributeName": "user_email", "KeyType": "HASH"}
        ],
        "Projection": {"ProjectionType": "ALL"},
        "ProvisionedThroughput": {
          "ReadCapacityUnits": 5,
          "WriteCapacityUnits": 5
        }
      }
    }]'
```

Or if using on-demand billing:

```bash
aws dynamodb update-table \
  --table-name YourTableName \
  --attribute-definitions \
    AttributeName=user_email,AttributeType=S \
  --global-secondary-index-updates \
    '[{
      "Create": {
        "IndexName": "user_email-index",
        "Keys": [
          {"AttributeName": "user_email", "KeyType": "HASH"}
        ],
        "Projection": {"ProjectionType": "ALL"}
      }
    }]'
```
