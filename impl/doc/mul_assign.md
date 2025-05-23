# What `#[derive(MulAssign)]` generates

This code is very similar to the code that is generated for `#[derive(Mul)]`.
The difference is that it mutates the existing instance instead of creating a
new one.

You can add the `#[mul_assign(forward)]` attribute if you don't want the same
semantics as `Mul`.
This will instead generate a `MulAssign` implementation with the same semantics
as `AddAssign`.




## Tuple structs

When deriving `MulAssign` for a tuple struct with two fields like this:

```rust
# use derive_more::MulAssign;
#
#[derive(MulAssign)]
struct MyInts(i32, i32);
```

Code like this will be generated:

```rust
# struct MyInts(i32, i32);
impl<__RhsT: Copy> derive_more::core::ops::MulAssign<__RhsT> for MyInts
    where i32: derive_more::core::ops::MulAssign<__RhsT>
{
    fn mul_assign(&mut self, rhs: __RhsT) {
        self.0.mul_assign(rhs);
        self.1.mul_assign(rhs);
    }
}
```

The behaviour is similar with more or less fields, except for the fact that
`__RhsT` does not need to implement `Copy` when only a single field is present.




## Regular structs

When deriving `MulAssign` for a regular struct with two fields like this:

```rust
# use derive_more::MulAssign;
#
#[derive(MulAssign)]
struct Point2D {
    x: i32,
    y: i32,
}
```

Code like this will be generated:

```rust
# struct Point2D {
#     x: i32,
#     y: i32,
# }
impl<__RhsT: Copy> derive_more::core::ops::MulAssign<__RhsT> for Point2D
    where i32: derive_more::core::ops::MulAssign<__RhsT>
{
    fn mul_assign(&mut self, rhs: __RhsT) {
        self.x.mul_assign(rhs);
        self.y.mul_assign(rhs);
    }
}
```

The behaviour is again similar with more or less fields, except that `Copy`
doesn't have to be implemented for `__Rhst` when the struct has only a single
field.




## Skipping fields

Struct marked with `#[mul_assign(forward)]` can hold zero sized fields, most
commonly `PhantomData`. A field containing a ZST can be skipped using the
`#[mul_assign(skip)]` attribute like this

```rust
# use core::marker::PhantomData;
# 
#[derive(MulAssign)]
#[mul_assign(forward)]
struct TupleWithZst<T>(i32, #[mul_assign(skip)] PhantomData<T>);

#[derive(MulAssign)]
#[mul_assign(forward)]
struct StructWithZst<T> {
    x: i32,
    #[mul_assign(skip)]
    _marker: PhantomData<T>,
}
```




## Enums

Deriving `MulAssign` for enums is not (yet) supported.
This has two reason, the first being that deriving `Mul` is also not implemented
for enums yet.
The second reason is the same as for `AddAssign`.
Even if it would be deriving `Mul` was implemented it would likely return a
`Result<EnumType>` instead of an `EnumType`.
Handling the case where it errors would be hard and maybe impossible.
