# Aspect Oriented Programming (AOP) for Rust

```
Changchun Fan, Yijun Yu
Central Software Institute & Trustworthiness Software Engineering
2012 R&D Lab, Huawei Technologies, Co.
```

[TOC]

## The needs of AOP

Aspect-oriented programming (AOP) is a programming paradigm that aims to increase modularity by allowing the separation of cross-cutting concerns. It does so by adding additional behavior to existing code--an advice-- instead of modifying the code itself, it separately specifies which code is modified via a \"pointcut\" specification. For example, "log all function calls when the function\'s name begins with \'set\'\". This allows behaviors that are not central to the business logic, such as logging, to be added to a program without cluttering the code that is core to the functionality.

AOP entails breaking down program logic into distinct parts, so-called concerns, which are cohesive areas of functionality. Nearly all programming paradigms support separation of concerns into independent entities by providing certain abstractions e. g., functions, procedures, modules, classes, methods. These abstract constructs can be used for implementing these concerns and make them composable. However, often some concerns \"cut across\" multiple abstractions in a program, and defy the purpose of separation. Such concerns are called cross-cutting concerns or horizontal concerns.

In the above example, logging exemplifies a crosscutting concern because a logging strategy necessarily affects every logged part of the system. Logging thereby crosscuts all logged classes and methods.

An implementation of AOP has been provided for Java, which is called AspectJ. However, for Rust there has not been many AOP implementations. One notable example is aspect-rs, which implements some basic AOP concepts. However, we find more cases where a full features of AOP like AspectJ  is missing. 

To support AspectJ-like AOP in general, we hope to design a mechanism that can be used in Rust. This documents highlights basic ingredients for such a prototype. We submit it as an initial MFC to the Rust community. 

## Implementation

Assume that our goal is to add a log after the `std::sync::mpsc::Sender\<T>::send()` method. There is an alternative for the chosen example design. 

### Alternative: use procedure macro

```rust
#[auto_log]
fn bussiness() {
    // ...
    ch.send(msg);
    // ...
    bus.send(obj);
}
// after macro expansion
fn bussiness() {
    // ...
    ch.send(msg);
    if type_name_of_val(&ch).starts_with(“std::sync::mpsc::Sender”) {
        log("channel::send is called")
    }
    // ...
    bus.send(obj);
    if type_name_of_val(&bus).starts_with(“std::sync::mpsc::Sender”) {
        log("channel::send is called")
    }
}
```

It works fine in theory. Users could simply add attributes to the business code to replace the log statement in the original code. 

The drawbacks are:

1.  Although writing attribute statements is easier than writing log statements directly, it is still not convenient enough. In the case
    of many functions, one needs to change a lot of code. That change is too intrusive to the business logic.
    
2.  Another disadvantage is that there is no type information at the macro expansion phase, so if type is concerned, one has to use runtime type checking, which could introduce additional performance overhead.

 **Idea for improvement**: create new tools to do automatic code generation.

We can represent all the aspect information as a separate file and use a separate tool to integrate the aspect logic with business logic. We want to make it as non-intrusive as possible. We want to reduce the additional performance overhead associated with runtime type checking in final generated code. The basic ideas are three steps: 

1) do the type checking for the original project, and find out where to insert the aspect code;
2) insert code pieces at precise locations;
3) recompile the project again.

## Usage

The example implementation includes the following two tools:

-   `cargo-aspect`

> This is a cargo subcommand. This is the tool programmer used directly.

-   and a modified `rustc` toolchain

> This is a modified `rustc` compiler. We added a new command line option: `-Z aop-inspect="..."`. This tool is driven by the `cargo-aspect` command, and is used for finding out where to insert the aspect code. Note that for the ease of use, we would create a new toolchain in `rustup` as follows:
>
> ```
>rustup toolchain link AOP /my/custom/rustc/toolchain
> ```

When the above tools are ready, they can be used as follows:

1. Write an `Aspect.toml` file in your current project with the following format:

   ```rust
   name = "test aspect"
   [[pointcuts]]
   condition = "call _x.unwrap()"
   advice = 'dbg!(_x).unwrap()'
   ```

2. compile the project using `cargo aspect` command.

`cargo aspect` does the following：

1.  read the contents of `Aspect.toml` file;

2.  make a full copy of `src` folder into `src-saved` folder;

3.  call `cargo +AOP rustc -- -Z aop-inspect="call _.unwrap()"` to find the code location where the `unwrap()` method was called. The results are stored as a `RUST_ASPECT_OUTPUT.txt` file in the output directory;
    
4.  read the search results, modify the source code, insert this line `println!("function unwrap is called");` at the end of the
    concern;
    
5.  compile the project again using the default toolchain.

The fields of "condition" and "advice" in the `Aspect.toml` file are actually a kind of DSL. The syntax can be improved in the future. We will describe the current design as follows.

### Syntax of the concerns

A "condition" describes what code you want to look for. The syntax it supports is as follows:

```bnf
Condition := PDecl (‘where’ Constraint (‘&&’ Constraint)*)?
PDecl := 
‘call’ Name ‘(’ Args ‘)’
| ‘call’ Name.Name ‘(‘ Args ‘)’
| ‘enter’ Path
| ‘exit’ Path
Constraint :=
     Var ‘:’ Path
| Var ‘impl’ Path
Args := ‘*’ | Name (‘,’ Name)*
Path := (Ident | ‘<’ | ‘>’ | ‘::’)+
Name := Var | Ident
Var  := ‘_’ Ident
```

The following are examples：

| Examples                                      | Description                                                  |
| --------------------------------------------- | ------------------------------------------------------------ |
| `call spawn()`                                | a function call<br/>1.	the name is “spawn”<br/>2.	there is no arguments |
| `call x.iter()`                               | a method call<br/>1.	the receiver’s name is “x”<br/>2.	the method name is “iter”<br/>3.	there is no arguments |
| `call _x.iter() where _x : Vec<i32>`          | a method call<br/>1.	the receiver’s name is any string<br/>2.	the method name is “iter”<br/>3.	there is no argument<br/>4.	the receiver’s type is “Vec<i32>” |
| `call _s.find(_c) where _s: &str && _c: char` | a method call<br/>1.	the receiver’s name is any string<br/>2.	the method name is “find”<br/>3.	the arguments count is 1<br/>4.	the receiver’s type is “&str”<br/>5.	the argument’s type is char |

## Explanation of the `cargo-aspect` subcommand

Synopsis: `cargo aspect` <project>

This `project` is a normal executable project, and the cargo subcommand requires that the executable's name to start with "cargo-".

The main process are described below:

-   `main()` - This is the main entry. Three main functions are called:

1.  `config::parse_config()`: read the contents of the `Aspect.toml`.
    Using `serde`-based deserialization to parse the config file.

2.  `src_mgr::backup_src()`: backup source code `src` folder

3.  `make::build_proj(&c)`:

    a.  compile the project using our modified AOP `rustc` toolchain, using the command:
    
    ````bash
    cargo +AOP rustc -- -Z aop-inspect="inspect_str" 
    ````

    b.  read the output result in `RUST_ASPECT_OUTPUT.txt;`
    
    c.  according to the config file to determine the advice code and where to insert it.

### Semantic Search

#### Add compilation options

The additional options for using the rustc compiler are defined in `compiler/rustc_session/src/options.rs`. These options are not yet from stable versions, so we have to add them into the `-Z` option group, which is related to debugging and printing information.

Like other options, we can add an option called `aop_inspect` that takes a string as an argument.

After compilation, we can use `rustc --Z help` to see all the available options.

#### The driver

The entry point for `rustc` is in `rustc/src/main.rs`, and this function immediately calls `rustc_driver::main()`. We need to add new logic to the `rustc_driver` project.

In `rustc_driver/src/lib.rs`, there is a long `run_compiler()` function in which the main flow of the compiler is executed. Because we need type information of the program, we must wait until the type analysis is done to do our lookups. So we can add new logic after `tcx.analysis(LOCAL_CRATE)` statement.

Here we add a function `process_aspects` that first determines whether the user has used the `-Z aop-inspect` option, takes the string for that option, and then processes it. Because of the complexity of the process, let's create a new file called `pointcut.rs` and write the main logic therein.

On the `rustc_driver` side, the function `pointcut::search_pointcuts` is called to retrieve all search results. There are two arguments of the function, one is user-specified command line argument, and the other is `rustc_middle::ty::TyCtxt`.

This `TyCtxt` is the most important data structure. It contains all the information we want, including syntax tree structure, source location, MIR information, type information for each node, and a lot of others. All the information we need is found there.

#### Parsing condition string

The user's command-line argument is parsed using the function `Pointcut::parse()`. The result is a `Pointcut` struct. This type can be thought of being an AST structure. It contains `PDecl` and a set of constraints.

```rust
struct Pointcut {

    decl: PDecl,

    conditions: Vec\<Constraint>,

}
```

The parse process consists of two steps:

1. Lexer: The string is first converted to a token stream, which is defined as follows：

   ```rust
   enum PointcutToken {
       Dot,        // .
       LParen,     // (
       RParen,     // )
       Star,       // \*
       LAngle,     // \<
       RAngle,     // >
       Colon,      // :
       ColonColon, // ::
       AndAnd,     // &&
       Comma,      // ,
       Impl,       // impl
       Enter,      // enter
       Exit,       // exit
       Call,       // call
       Where,      // where
       Find,       // find
       Name(String),
   }
   ```

2.  Parser: That is to convert the token stream into a tree structure according to the grammar defined above.

#### Verify the AST

The generated AST might contains invalid nodes. For example, if the user provides `call _x.iter() where _y: i32`. This option is syntactically correct but semantically incorrect, because the `_y` variable does not appear in the previous expression, and we don't know what `_y` stands for. In such cases, we should verify that the user-specified Pointcut is valid before proceeding to the next step.

This step is done in the `Pointcut::validate()` function. It consists two steps:

1.  Collect variable names in `PDecl`:

> Note that we only collect words that begin with an underscore. Only these names are treat as variables subject to additional constraints.

2.  Check all constraints for names that don't appear before;

3.  If `PDecl` contains a variable that is not used in the Constraints, we also report an error

#### Traverse HIR

Take the most common scenario as the example: look up a function call. The most appropriate data structure for finding function calls would be the HIR, which also has type checking information. We can get HIR in `TyCtxt` by `tcx.hir().krate()`. The way to iterate through the HIR is to implement the trait `intravisit::Visitor`.

We designed a `struct FindCallExprs` to find a function call, and members `tcx: TyCtxt` and `pc: Pointcut` are input, the members found respectively: `Vec<Span>` is the output information. This type needs to implement the trait `intravisit::Visitor`. In this `impl` block, we implemented several functions:

-   `visit_fn`: This function can find the entry and exit point of a function.
    
-   `visit_expr`: It is mainly used for function/method call lookup.
    `ExprKind::call` represents a function call, and `ExprKind::MethodCall` represents a method call. The information of
    the expression and the condition given by the user can be compared.
    
-   `visit_stmt` with the primary purpose to record the position of the statement where the current function call is located. The reason is that a function call might be a subexpression inside a statement, and the expression's location is not a proper
    location to insert code. We can only insert code after or before the whole statement, otherwise a syntax error may be introduced.
    
-   `visit_block`: There is a special case where rust allows the last expression of a block to be the value of the block. If the function
    call appear in the trailing expression, we need this function to keep track of the position of the whole trailing expression.

#### Compare the types of HIR nodes

The type of the HIR node is obtained by the function `FindCallExprs::sema_ty()`. Firstly, it gets the body id of current HIR function body. Secondly, it uses `TyCtxt::typeck_body()` to get the type of all nodes in the function body. Finally, it uses the `node_type()` method to get the type of a specific HIR node.

Once we have the type, the `to_string()` method is used to turn it into a string and compare it with the user-specified type.

#### Save positions

The locations found in `search_pointcuts` are stored in a file, whose name is `crate_name + "RUST_ASPECT_OUTPUT.txt"`.

## Unimplemented features

1.  the constraint doesn't support `var impl trait`:

> I guess this feature should be implemented based on `TyCtxt::type_implements_trait`. But I have some trouble to get the
> DefId of a trait from a string.

2.  Only "function call" and "method call" pattern are implemented in the condition;
    
3.  There are still difficulties to insert new code without introducing syntax errors. I tried two ways:
    
    a.  add new statement after one statement, there are a few corner cases
    
    b.  replace the concerned expression with a macro expansion. For example, replace the `s.send("msg")` to `log!(s).send("msg")`. And the `log!` macro will do the real logging which can be implemented like the `std::dbg!` macro.

