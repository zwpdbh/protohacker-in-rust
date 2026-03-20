# Static vs Dynamic Dispatch

```rust
trait UserService {
  async fn get_user(&self, id: &str) -> Option<User>;
}

// Suppose we want to write a function in which it
// accepts any type that implement `UserService`
// Q: what should the type of this function argument be?
async fn use_service(service: ???) {
  let user = service.get_user("user-123").await;
  // ...
}
```

Two ways to do polymorphism in Rust

- static dispatch -- generics with trait bounds: `T: UserService`
- dynamic dispatch -- trait objects with trait bounds: `Box<dyn UserService>`
  - It doesn't know the concrete type until runtime.

## Static Dispatch

- Different types implement same `trait`.
- No runtime overhead
- Require us to have to know all the types in advance.

## Dynamic Dispatch

- It is needed when a object's type could not be known until runtime.

## References

- [Rust’s most complicated features explained](https://www.youtube.com/watch?v=9RsgFFp67eo)
