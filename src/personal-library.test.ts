import { expect, it } from "vitest";
import { restoreSearch, type SearchState } from "./personal-library";
it("restores all saved filters without silently broadening a removed-root search", () => {
  const state: SearchState = {
    text: "PDFs from last week",
    category: "documents",
    root_id: 42,
    source: "example.com",
    cleanup: "large",
    sort: "size",
    favourites: true,
  };
  expect(restoreSearch(state, [42])).toEqual(state);
  expect(() => restoreSearch(state, [41])).toThrow("removed folder");
  expect(state.root_id).toBe(42);
});
it("restores native null optional filters as unset and keeps relative date text", () => {
  const state = JSON.parse(
    '{"text":"images from last week","category":null,"root_id":null,"source":null,"cleanup":null,"sort":"recent","favourites":false}',
  ) as SearchState;
  expect(restoreSearch(state, [])).toEqual({
    text: state.text,
    category: undefined,
    root_id: undefined,
    source: undefined,
    cleanup: undefined,
    sort: "recent",
    favourites: false,
  });
});
