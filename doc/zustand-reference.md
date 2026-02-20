# Zustand v5 Reference

> State management for GDML Studio frontend.
> Sources: [Zustand docs](https://zustand.docs.pmnd.rs/), [GitHub](https://github.com/pmndrs/zustand), [v5 migration](https://zustand.docs.pmnd.rs/migrations/migrating-to-v5)

---

## 1. Core API

### Creating a Store

```typescript
import { create } from 'zustand'

// v5 TypeScript: must use curried form create<T>()()
interface AppState {
  count: number
  loading: boolean
  increment: () => void
  reset: () => void
}

const useStore = create<AppState>()((set) => ({
  count: 0,
  loading: false,
  increment: () => set((s) => ({ count: s.count + 1 })),
  reset: () => set({ count: 0, loading: false }),
}))
```

The creator function receives `(set, get, store)`.

### `set` — Updating State

```typescript
// Shallow merge (default) — only updates specified fields
set({ count: 5 })

// Updater function — access previous state
set((state) => ({ count: state.count + 1 }))

// Replace entire state (second arg = true, must provide ALL fields)
set({ count: 0, loading: false, increment: ..., reset: ... }, true)
```

**Important:** `set` does a shallow merge, NOT deep merge. Nested objects must be spread manually:

```typescript
// Correct
set((s) => ({ nested: { ...s.nested, field: 'new' } }))

// WRONG — mutation, won't trigger re-renders
set((s) => { s.nested.field = 'new' })
```

### `get` — Reading State Inside the Store

```typescript
const useStore = create<MyState>()((set, get) => ({
  count: 0,
  doubled: () => get().count * 2,
  asyncAction: async () => {
    const current = get().count
    const res = await fetch(`/api?count=${current}`)
    set({ count: (await res.json()).value })
  },
}))
```

### Selectors — Preventing Re-renders

```tsx
// GOOD — only re-renders when count changes
const count = useStore((s) => s.count)

// GOOD — action refs are stable, won't cause re-renders
const increment = useStore((s) => s.increment)

// BAD — subscribes to entire store, re-renders on ANY change
const state = useStore()
```

### Accessing State Outside React

```typescript
// Read
const current = useStore.getState().count

// Write
useStore.setState({ count: 10 })
useStore.setState((s) => ({ count: s.count + 1 }))

// Get initial state (as defined at creation)
const initial = useStore.getInitialState()

// Subscribe to changes outside React
const unsub = useStore.subscribe((state, prev) => {
  console.log('changed:', prev, '->', state)
})
unsub() // unsubscribe
```

---

## 2. Patterns

### Async Actions

```typescript
const useStore = create<MyState>()((set) => ({
  data: null,
  loading: false,
  error: null,
  fetchData: async () => {
    set({ loading: true, error: null })
    try {
      const res = await fetch('/api/data')
      const data = await res.json()
      set({ data, loading: false })
    } catch (e) {
      set({ error: (e as Error).message, loading: false })
    }
  },
}))
```

### Resetting to Initial State

```typescript
const initialState = {
  count: 0,
  items: [],
}

const useStore = create<State>()((set) => ({
  ...initialState,
  addItem: (item) => set((s) => ({ items: [...s.items, item] })),
  reset: () => set(initialState),
}))
```

### Store Slices (Splitting Large Stores)

```typescript
import { StateCreator } from 'zustand'

interface BearSlice { bears: number; addBear: () => void }
interface FishSlice { fishes: number; addFish: () => void }

const createBearSlice: StateCreator<
  BearSlice & FishSlice, [], [], BearSlice
> = (set) => ({
  bears: 0,
  addBear: () => set((s) => ({ bears: s.bears + 1 })),
})

const createFishSlice: StateCreator<
  BearSlice & FishSlice, [], [], FishSlice
> = (set) => ({
  fishes: 0,
  addFish: () => set((s) => ({ fishes: s.fishes + 1 })),
})

// Combine
const useStore = create<BearSlice & FishSlice>()((...a) => ({
  ...createBearSlice(...a),
  ...createFishSlice(...a),
}))
```

### Actions Outside the Store

```typescript
// Store has only state
export const useStore = create(() => ({ count: 0, text: '' }))

// Actions at module level — no hook needed to call them
export const inc = () => useStore.setState((s) => ({ count: s.count + 1 }))
export const setText = (t: string) => useStore.setState({ text: t })
```

---

## 3. Performance

### Selector Best Practices

```tsx
// 1. Always use selectors — never call useStore() bare
const count = useStore((s) => s.count)

// 2. Select actions separately (stable references)
const increment = useStore((s) => s.increment)

// 3. For multiple fields, use useShallow
import { useShallow } from 'zustand/react/shallow'

const { count, name } = useStore(
  useShallow((s) => ({ count: s.count, name: s.name }))
)

// 4. Cache fallback values outside the selector
const EMPTY: Item[] = []
const items = useStore((s) => s.items ?? EMPTY)
```

### When to Use `useShallow`

Use when selector returns a **new reference** each render (objects, arrays, derived arrays):

```tsx
// Without useShallow — infinite loop in v5!
const [val, setVal] = useStore((s) => [s.val, s.setVal])

// With useShallow — safe
const [val, setVal] = useStore(useShallow((s) => [s.val, s.setVal]))
```

`useShallow` compares by shallow equality instead of `Object.is`.

---

## 4. Middleware

### `persist` — LocalStorage Persistence

```typescript
import { persist, createJSONStorage } from 'zustand/middleware'

const useStore = create<MyState>()(
  persist(
    (set) => ({ count: 0, inc: () => set((s) => ({ count: s.count + 1 })) }),
    {
      name: 'my-storage',                              // required: storage key
      storage: createJSONStorage(() => sessionStorage), // default: localStorage
      partialize: (state) => ({ count: state.count }),  // only persist some fields
      version: 1,                                       // schema version
      migrate: (persisted, version) => {                // migration function
        if (version === 0) { /* upgrade */ }
        return persisted as MyState
      },
    }
  )
)
```

### `devtools` — Redux DevTools

```typescript
import { devtools } from 'zustand/middleware'

const useStore = create<MyState>()(
  devtools(
    (set) => ({
      count: 0,
      inc: () => set(
        (s) => ({ count: s.count + 1 }),
        undefined,        // 2nd arg: must be undefined in v5
        'count/increment' // 3rd arg: action name in DevTools
      ),
    }),
    { name: 'MyStore', enabled: process.env.NODE_ENV === 'development' }
  )
)
```

### `immer` — Mutable Syntax for Immutable Updates

```typescript
import { immer } from 'zustand/middleware/immer'

const useStore = create<MyState>()(
  immer((set) => ({
    nested: { value: 0 },
    update: () => set((state) => {
      state.nested.value += 1  // direct mutation is safe with immer
    }),
  }))
)
```

### `subscribeWithSelector` — Fine-Grained Subscriptions

```typescript
import { subscribeWithSelector } from 'zustand/middleware'

const useStore = create<MyState>()(
  subscribeWithSelector((set) => ({ count: 0 }))
)

// Subscribe to a specific field
const unsub = useStore.subscribe(
  (s) => s.count,
  (count, prevCount) => console.log(prevCount, '->', count),
  { fireImmediately: true }
)
```

### Combining Middleware (Order Matters)

`devtools` must be **outermost**:

```typescript
// Correct order: devtools > persist > subscribeWithSelector > immer
devtools(persist(subscribeWithSelector(immer((set) => ({ ... })))))
```

---

## 5. TypeScript

### Curried Create (Required in v5)

```typescript
// v5: create<Type>()((set) => ...)  — note the double ()()
const useStore = create<MyState>()((set) => ({ ... }))
```

### StateCreator for Slices

```typescript
import { StateCreator } from 'zustand'

const createSlice: StateCreator<
  FullState,        // combined state type
  [],               // middleware mutators in
  [],               // middleware mutators out
  SliceType         // this slice's return type
> = (set, get) => ({ ... })
```

### Middleware Mutator Types

| Middleware | Mutator type |
|-----------|-------------|
| `devtools` | `['zustand/devtools', never]` |
| `persist` | `['zustand/persist', PersistedState]` |
| `immer` | `['zustand/immer', never]` |
| `subscribeWithSelector` | `['zustand/subscribeWithSelector', never]` |

---

## 6. v5 Breaking Changes (from v4)

| Change | Migration |
|--------|-----------|
| Default exports removed | Use named: `import { create } from 'zustand'` |
| React 18+ required | Upgrade React |
| `shallow` as 2nd selector arg removed | Use `useShallow` or `createWithEqualityFn` |
| Selectors returning new refs can loop | Wrap with `useShallow` |
| `setState(partial, true)` stricter | Must provide complete state when replacing |
| `persist` no longer stores initial state on creation | Explicitly set state after creation if needed |
| `useShallow` import path | `import { useShallow } from 'zustand/react/shallow'` |
| `createWithEqualityFn` import path | `import { createWithEqualityFn } from 'zustand/traditional'` |
