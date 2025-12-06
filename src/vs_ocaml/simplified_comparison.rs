#![allow(unused)]
use crate::Result;

// Simplified comparison: OCaml functor vs Rust equivalent

// OCaml version:
/*
module type ITERABLE = sig
  type 'a t
  val iter : ('a -> unit) -> 'a t -> unit
end

module type S = sig
  val f : string list -> unit
end

module MakeIterPrint (Dep : ITERABLE) = struct
  let f container = Dep.iter (fun s -> print_endline s) container
end

module ListProcessor = MakeIterPrint(List)
*/

// Rust equivalent - simplified version
trait Iterable<T> {
    fn iter<F>(f: F, container: Self)
    where
        F: FnMut(T);
}

// Implementation for Vec (like OCaml's List)
impl<T> Iterable<T> for Vec<T> {
    fn iter<F>(mut f: F, container: Self)
    where
        F: FnMut(T),
    {
        for item in container {
            f(item);
        }
    }
}

// Simplest functor-like structure
struct MakeIterPrint;

impl MakeIterPrint {
    fn f<T, Container>(container: Container)
    where
        Container: Iterable<T>,
        T: std::fmt::Debug,
    {
        Container::iter(|item| println!("{:?}", item), container);
    }
}

// Usage
fn simple_example() {
    // In OCaml: ListProcessor.f [4; 5; 6]
    MakeIterPrint::f(vec![4, 5, 6]);  // Rust equivalent
}

// Or with a more functor-like approach - remove the problematic implementation
// The issue was trying to have unconstrained type parameter T
// Instead, let's define trait with associated types like in the main implementation

trait IterableContainer {
    type Item;
    fn iter<F>(f: F, container: Self)
    where
        F: FnMut(Self::Item);
}

// Implementation for Vec with associated types
impl<T> IterableContainer for Vec<T> {
    type Item = T;

    fn iter<F>(mut f: F, container: Self)
    where
        F: FnMut(Self::Item),
    {
        for item in container {
            f(item);
        }
    }
}

struct FunctorProcessor<Dep>(std::marker::PhantomData<Dep>);

// This implementation works without unconstrained type parameters
impl<Dep> FunctorProcessor<Dep>
where
    Dep: IterableContainer,
    Dep::Item: std::fmt::Debug,
{
    fn f(container: Dep) {
        Dep::iter(|item| println!("{:?}", item), container);
    }
}

// Even more simplified version - most direct translation
trait FunctorInterface<T> {
    fn process(&self, items: Vec<T>)
    where
        T: std::fmt::Debug;
}

struct ListProcessor;

impl<T> FunctorInterface<T> for ListProcessor
where
    T: std::fmt::Debug,
{
    fn process(&self, items: Vec<T>) {
        for item in items {
            println!("{:?}", item);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_example_test() -> Result<()> {
        let processor = ListProcessor;
        processor.process(vec![1, 2, 3]);
        Ok(())
    }
}