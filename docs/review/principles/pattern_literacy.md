# Principle: Pattern literacy

Doctrine 5 in `../doctrine.md`. The patterns are the Gang of Four catalog
(Gamma, Helm, Johnson, Vlissides, *Design Patterns*, 1994) and the
architectural patterns that grew beside it: MVC and its descendants,
Mediator-centred architectures, entity systems. Pattern literacy is the
ability to look at a chunk of code and see that it is an unnamed, partial,
or ad-hoc rendering of a pattern the book already names, and to say what
the code would look like if it were that pattern on purpose. When a big
chunk of code can be replaced with a pattern that makes it more extensible,
the code heads toward that pattern.

This is part of correctness, not style. Ad-hoc code that works today and
costs N edits to extend tomorrow is incorrect with respect to the declared
future (five ships, four planets, open-set enemies, panels on any face).
When weighing expediency against correctness, the reviewer must be able to
name the pattern the expedient code is failing to be.

This muscle has not been flexed much in this repo. Expect findings; the
owner has said a repo-wide pass on this lens is a separate piece of work.
This brief reviews the diff, but where a hunk extends an existing ad-hoc
structure, say so and name the pattern the whole structure wants to be.

## The catalog, in Rust terms

Rust realizes the patterns differently from the book's C++ and Smalltalk.
A literate reviewer knows both the book's name and the Rust shape, and
does not demand Java ceremony where a language feature already is the
pattern. Verify the repo sites named here with Serena; they are examples,
not a claim that every one is implemented well.

| GoF pattern | Intent | Rust shape | Where it lives or wants to live here |
|---|---|---|---|
| Strategy | Interchangeable algorithm behind one interface | Trait object for open sets; enum + match for closed sets; closures for one-method strategies | Ship weapons (one per hull), stereo convergence policy, room population policy |
| State | Behavior changes with state; transitions explicit | Enum with `can_transition_to`; typestate for compile-time-legal sequences | `GamePhase` FSM; dormancy (dormant/active) on pooled entities |
| Mediator | Peers talk through one hub, never to each other | One struct owning the routing; signals in, private state mutation | `GameManager` + `RunState`; the views/ui wall is the Mediator's contract |
| Observer | Subscribers notified of events | Godot signals with typed constants; channels in the model | Every signal route; HUD tints on damage |
| Command | Requests as objects: queue, undo, replay | Enum of actions or boxed `FnOnce`; deferred via `call_deferred` | Deferred dormancy flips; input to ship intents; save/replay |
| Builder | Stepwise construction of a complex object | Struct with chained methods returning `Self`; `build()` returns the product | `RoomBuild`; level construction stages; the credits roll |
| Abstract Factory | Families of related products | Trait with several constructors; a per-family struct implementing it | Per-planet kits (panel families per planet); enemy rosters per chapter |
| Factory Method | Subtype decides what to construct | Trait method returning `Self` or `Box<dyn T>` | Scene minting via `SceneId`; entity pools materializing manifests |
| Prototype | New objects by cloning an exemplar | `Clone` on a template; pools stamped from one manifest | Faucet Principle pools: preallocate from the manifest, then flip |
| Singleton | One instance, global access | A smell in Rust; prefer ownership at the root and passing down; Godot autoloads are the one accepted form | `GameManager` as autoload; `GameOptions` broadcast |
| Flyweight | Share intrinsic state, key extrinsic | Interned handles; `&'static` or arena-backed data behind a `Copy` key | `EnemyKey`, `SceneId` interners; kit pools |
| Adapter | Make an interface fit another | A newtype wrapping the foreign type with the wanted methods | The Godot boundary: `Variant` to domain newtypes, `GString` to keys |
| Facade | One simple door over a subsystem | A module with one `pub` entry and private internals | `LevelGraph` (opaque, method-only); the catalog's scene accessor |
| Bridge | Separate abstraction from implementation so both vary | Trait in the model crate, impls in the shell crate | The model/shell split itself: `void-logic` abstractions, `void-nodes` realizations |
| Decorator | Add behavior by wrapping | A newtype implementing the same trait and delegating | Damage modifiers, shield layers, stereo tint escalation |
| Composite | Tree of parts treated uniformly | Recursive enum or trait over children | Godot scene tree; rooms containing cells containing occupants |
| Proxy | Stand-in that controls access | Handle types with validity checks | `Gd<T>` handles; dormant entities as proxies for live ones |
| Chain of Responsibility | Pass a request along handlers until one takes it | Iterator over handlers returning `Option` | Hit resolution: shield, then hull, then destruction |
| Iterator | Sequential access without exposing the container | `impl Iterator`; petgraph walkers | `LevelGraph::visible_from`; roster iteration |
| Template Method | Skeleton with overridable steps | Trait with default methods calling required ones | Level construction stages; pipeline oracle, plan, apply |
| Visitor | Operations over a structure without changing it | Trait with one method per variant; or enum + match when the set is closed | Level manifest passes: population, culling, minimap |
| Memento | Capture and restore state | Serializable snapshot struct, separate from the live object | Save files; `RunState` snapshots |
| Interpreter | A grammar and its evaluator | Data-driven enums parsed by serde, evaluated by one function | Roster TOML: switches, curves, emitters |

Architectural patterns:

| Pattern | Roles | Where it lives here |
|---|---|---|
| MVC / MVP | Model holds state and rules; View renders and never mutates; Controller or Presenter routes input to the model and state to views | Model = `void-logic` + `RunState`; Views = `nodes/views/` and `nodes/ui/`; Controller = `GameManager` signal routes. Views importing each other, or a view mutating state, breaks the pattern |
| Entity pools | Preallocated entities, activated by flag | Faucet Principle; not a full ECS and must not drift into one |
| Pipeline / Pipes and Filters | Stages with typed products between them | `make assets`: install, import, probe, credits, audit; oracle, plan, apply inside the converters |

## Checks

1. **Name the latent pattern.** For every chunk of new or changed code
   larger than a helper, ask which pattern it is trying to be. A `match`
   over kinds that dispatches behavior is Strategy or State. A struct that
   fans messages between peers is Mediator. A constructor with many
   optional parts is Builder. A bag of callbacks with flags is Observer done
   by hand. A `bool` state machine is State without transitions. Name it
   and cite the chunk.
2. **The extensibility test.** For the declared future (the next ship, the
   next planet, the next enemy, the next kit, the next face), count the
   sites this code makes that addition touch. If N sites where the named
   pattern would make it one, that is the finding: list the N sites and
   the pattern. This is the check that ties Doctrine 5 to Doctrine 4.
3. **Half a pattern.** Code that has the pattern's shape but not the part
   that pays: State whose transitions live in callers instead of the FSM;
   Builder whose product can still be constructed raw; Mediator that peers
   bypass; Strategy where the interface leaks the concrete type; Facade
   with a second door. Cite the missing part.
4. **Rust shape, not Java ceremony.** A pattern realized against the
   language is a Zen of the Tool failure. Do not demand a trait object for
   a closed set an enum handles; do not demand a Command struct where a
   closure is the command; do not demand a Singleton where ownership at the
   root works. The inverse is also a finding: Java-era ceremony (an
   abstract factory for one product, a Visitor over a two-variant enum)
   imported without need.
5. **Over-patterning.** A pattern is introduced only when a second concrete
   use exists or the declared future names it. A pattern with one use and
   no declared second is speculative generality; cross-report to
   `strategic_design.md`. The book's own warning applies: patterns add
   indirection, and indirection without a variation point to justify it is
   cost.
6. **Naming as literacy.** When a pattern is present on purpose, its name
   is in the type name or the doc comment (`RoomBuild` says Builder;
   `GamePhase` should say State machine). Vocabulary shared with the next
   reader is the point; a pattern that is present but unnamed is half the
   value.
7. **Architectural roles.** New UI, view, or controller code respects
   MVC roles: views render and never mutate, the mediator routes, the
   model decides. A view that reaches into another view, a view that
   mutates `RunState`, or a controller that renders is a finding under this
   brief as well as `encapsulation.md`.
8. **The whole structure, not the hunk.** When a hunk extends an existing
   ad-hoc structure, the finding names the whole structure and the pattern
   it wants to be, and says whether this diff moved toward it or further
   from it. That is how this lens feeds the repo-wide pass the owner has
   called for.

## Procedure

For each touched module, `get_symbols_overview` and read the whole file.
For each type with behavior, decide its pattern role. For each `match` on a
domain enum, `find_referencing_symbols` on the enum to count the dispatch
sites; that count is the extensibility test's raw material. For each
Godot signal, name its Observer roles. Finish with a table: module, pattern
present or latent, named or not, sites to extend.

## Severity guidance

Extensibility test fails (N sites for the next declared addition) where a
named pattern makes it one: BLOCKER. Half a pattern on a lifecycle, mediator,
or pool path: BLOCKER. MVC role violation: BLOCKER. Latent pattern unnamed
in new code: CONCERN with the pattern named. Java ceremony or
over-patterning: CONCERN. Missing name on a deliberate pattern: NIT.
