## Programming Style
- Prefer to use functional style over object-oriented style when possible.
- Pay attention to data flow and state management. Prefer immutable data structures when possible.
- Follow SSOT (Single Source of Truth) principle. Avoid duplicating data and logic across the codebase.
- Write modular code. Break down large functions into smaller, reusable functions. But avoid creating too many small functions that make the code hard to follow.
- Keep related data and functions together. Prefer to use AoS (Array of Structures) over SoA (Structure of Arrays) when it improves code clarity.
- Keep performance in mind, but do not sacrifice code readability for minor performance gains.
