#![allow(unused)]
use crate::Result;
// Define the interface (like the OCaml signature)
// In OCaml:  val iter : ('a -> unit) -> 'a t -> unit
// This takes a function and a container
trait Iterable<T> {
    fn iter<F>(f: F, container: Vec<T>)
    where
        F: FnMut(T);
}

// Define the output interface
trait S<T> {
    fn f(&self, container: T);
}

struct MakeIterPrint<Dep> {
    dep: Dep, // The dependency
}

// Implement Iterable for a standalone function-like implementation
struct ListIterable;
impl<T> Iterable<T> for ListIterable {
    fn iter<F>(mut f: F, container: Vec<T>)
    where
        F: FnMut(T),
    {
        for item in container {
            f(item);
        }
    }
}

// Try to mimic dependency injection from OCaml's functor pattern
// Now MakeIterPrint uses the dependency's iteration capability
// Generic parameters:
// Dep is the dependency type parameter (the "module" being "injected")
// T is the element type for the containers being iterated over
// S<Vec<T>> is the trait being implemented.
// This means we're implementing the S trait for the specific case where the container type is Vec<T>
impl<Dep, T> S<Vec<T>> for MakeIterPrint<Dep>
where
    Dep: Iterable<T>,
    T: std::fmt::Debug,
{
    fn f(&self, container: Vec<T>) {
        // Use the dependency's implementation to iterate over the container
        Dep::iter(|item| println!("{:?}", item), container);
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused)]
    use super::*;

    #[test]
    fn case01() -> Result<()> {
        let list_processor = MakeIterPrint::<ListIterable> { dep: ListIterable };
        list_processor.f(vec![4, 5, 6]); // Print this container using dependency's iteration
        Ok(())
    }
}
