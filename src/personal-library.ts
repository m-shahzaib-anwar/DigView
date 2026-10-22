import type { Category } from "./types";
export interface SearchState {
  text: string;
  category?: Category;
  root_id?: number;
  source?: string;
  cleanup?: string;
  sort: string;
  favourites: boolean;
}
export interface SavedSearch {
  id: number;
  name: string;
  state: SearchState;
}
export interface PersonalLibrary {
  favourite_ids: number[];
  searches: SavedSearch[];
}
export const emptyPersonal: PersonalLibrary = {
  favourite_ids: [],
  searches: [],
};
export function restoreSearch(
  state: SearchState,
  rootIds: number[],
): SearchState {
  if (state.root_id != null && !rootIds.includes(state.root_id))
    throw new Error(
      "This saved search uses a removed folder. Add/select the folder and save a new search.",
    );
  return {
    ...state,
    category: state.category ?? undefined,
    root_id: state.root_id ?? undefined,
    source: state.source ?? undefined,
    cleanup: state.cleanup ?? undefined,
  };
}
