/**
 * A search handed to the collection from somewhere else.
 *
 * The collection's search box belongs to `Library`, and until §109 the only
 * thing that filled it from outside was the requests view *inside* the
 * library. The Requests activity puts the room's requests beside the decks as
 * a surface of their own, with the collection underneath — and its "find"
 * has to reach a search box in another component. This is that reach: a
 * query and a counter, so asking for the same text twice still asks.
 */
export const asked = $state({ query: "", times: 0 });

/** Ask the collection to search for `query`. */
export function findInCollection(query: string): void {
  asked.query = query;
  asked.times += 1;
}
