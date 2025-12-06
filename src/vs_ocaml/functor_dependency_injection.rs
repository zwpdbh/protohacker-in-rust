#![allow(unused)]
use crate::Result;

// Let me rewrite this to better mimic the OCaml version and explain the differences

// Rust: Define the interface (trait) similar to OCaml signature
trait Iterable {
    type Item;
    type Container;

    fn iter<F>(f: F, container: Self::Container)
    where
        F: FnMut(Self::Item),
        Self: Sized;
}

// Rust: Output interface similar to OCaml's S signature
trait S<T> {
    fn f(&self, container: T);
}

// Rust: Functor-like implementation using generic struct with trait bounds
struct MakeIterPrint<Dep> {
    dep: Dep, // The injected dependency
}

// In OCaml, the functor signature looked like:
// module MakeIterPrint (Dep : Iterable) : S with type 'a t := 'a Dep.t
//
// In Rust, we implement this as a generic implementation with trait bounds:
impl<Dep> S<Dep::Container> for MakeIterPrint<Dep>
where
    Dep: Iterable, // Dep must implement Iterable with its own Item and Container types
    Dep::Item: std::fmt::Debug, // The item type must implement Debug for printing
{
    // The implementation uses the dependency's iter method
    fn f(&self, container: Dep::Container) {
        Dep::iter(|item| println!("{:?}", item), container);
    }
}

// Now let's implement specific containers to match the OCaml example
struct VecIterable<T>(std::marker::PhantomData<T>);

impl<T> Iterable for VecIterable<T> {
    type Item = T;
    type Container = Vec<T>;

    fn iter<F>(mut f: F, container: Self::Container)
    where
        F: FnMut(Self::Item),
    {
        for item in container {
            f(item);
        }
    }
}

struct ArrayIterable<T>(std::marker::PhantomData<T>);

impl<T> Iterable for ArrayIterable<T> {
    type Item = T;
    type Container = [T; 3]; // Fixed-size array for demonstration

    fn iter<F>(mut f: F, container: Self::Container)
    where
        F: FnMut(Self::Item),
    {
        for item in container {
            f(item);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused)]
    use super::*;

    #[test]
    fn test_list_iteration() -> Result<()> {
        let processor = MakeIterPrint {
            dep: VecIterable(std::marker::PhantomData::<i32>),
        };
        processor.f(vec![4, 5, 6]);
        Ok(())
    }

    #[test]
    fn test_array_iteration() -> Result<()> {
        let processor = MakeIterPrint {
            dep: ArrayIterable(std::marker::PhantomData::<i32>),
        };
        processor.f([7, 8, 9]);
        Ok(())
    }
}

// Comparison: Here's how OCaml's functors work vs Rust's trait bounds

/*
OCaml Functor Example:
=====================

module type Iterable = sig
  type 'a t
  val iter : ('a -> unit) -> 'a t -> unit
end

module type S = sig
  type 'a t
  val f : string t -> unit
end

(* Functor: parameterized module *)
module MakeIterPrint(Dep : Iterable) : S with type 'a t := 'a Dep.t = struct
  let f container = Dep.iter (fun s -> print_endline s) container
end

(* Usage: *)
module ListProcessor = MakeIterPrint(List)  (* Inject List module *)
module ArrayProcessor = MakeIterPrint(Array)  (* Inject Array module *)

Rust equivalent:
==============
- Traits = module signatures
- Generic implementations = functors
- Trait bounds = module parameters
- Associated types = type constraints (with type 'a t := 'a Dep.t)
- Generics allow compile-time polymorphism like functors

Key differences:
1. OCaml functors work at module level (compile-time), Rust traits are more flexible
2. OCaml has explicit module parameters, Rust uses trait bounds
3. OCaml's type constraints are more explicit, Rust uses associated types
4. Rust's trait solver does more inference automatically
*/

// Let me provide a more advanced example that shows how to handle the type equality constraint
// that OCaml's `with type 'a t := 'a Dep.t` provides:

trait Container {
    type Item;
    type Collection;
    fn iter<F>(collection: Self::Collection, f: F)
    where
        F: FnMut(Self::Item);
}

// Implementation for Vec
struct VecContainer<T>(std::marker::PhantomData<T>);
impl<T> Container for VecContainer<T> {
    type Item = T;
    type Collection = Vec<T>;

    fn iter<F>(collection: Self::Collection, mut f: F)
    where
        F: FnMut(Self::Item),
    {
        for item in collection {
            f(item);
        }
    }
}

// A more direct mapping to the OCaml functor pattern
struct FunctorApplication<C>(C);

impl<C: Container> S<C::Collection> for FunctorApplication<C>
where
    C::Item: std::fmt::Debug,
{
    fn f(&self, container: C::Collection) {
        C::iter(container, |item| println!("{:?}", item));
    }
}

#[cfg(test)]
mod comparison_tests {
    use super::*;

    #[test]
    fn functor_like_example() -> Result<()> {
        let vec_processor = FunctorApplication(VecContainer::<i32>(std::marker::PhantomData));
        vec_processor.f(vec![1, 2, 3]);

        Ok(())
    }
}
