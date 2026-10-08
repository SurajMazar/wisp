// Shared ESLint flat config for all TypeScript packages.
import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import tseslint from "typescript-eslint";

export default tseslint.config(
  {
    ignores: [
      "**/dist/**",
      "**/node_modules/**",
      "**/generated/**",
      "target/**",
      "build/**",
      "apps/macos/**",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.strict,
  {
    plugins: { "react-hooks": reactHooks },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "@typescript-eslint/no-explicit-any": "error",
      "@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_", varsIgnorePattern: "^_" }],
      "@typescript-eslint/consistent-type-imports": "error",
      "no-console": ["warn", { allow: ["warn", "error"] }],
    },
  },
  {
    // Scripts and tests index into data they've just validated; assertions keep them readable.
    files: ["scripts/**", "**/*.config.*", "**/tests/**"],
    rules: { "no-console": "off", "@typescript-eslint/no-non-null-assertion": "off" },
  },
);
