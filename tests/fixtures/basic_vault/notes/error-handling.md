---
title: Error Handling
tags:
  - backend
  - patterns
---

# Error Handling

Our error handling approach follows the Result pattern.

## HTTP Errors

We map domain errors to HTTP status codes in the [[api-design]] layer.

## Logging

All errors are logged with context. #observability
