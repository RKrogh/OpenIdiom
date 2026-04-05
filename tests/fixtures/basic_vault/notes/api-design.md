---
title: API Design Patterns
tags: [architecture, backend]
status: published
---

# API Design Patterns

This note covers our API design conventions.

## Authentication

We use [[auth-middleware]] for all protected endpoints.
See also [[error-handling|Error Handling Guide]] for how we handle auth failures.

## Rate Limiting

Rate limiting is handled at the gateway level. #backend #performance

## References

- [[non-existent-note]] for future reference
- [[subfolder-note]]
