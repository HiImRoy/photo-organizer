import { describe, expect, it } from "vitest";

import { emptyAssetFilter } from "../types";
import { fixtureAssetPage, visualFixtureFromSearch } from "./visual-fixture";

describe("visual fixture rating filters", () => {
  it("returns the selected star and every higher star", () => {
    const fixture = visualFixtureFromSearch("?visual-fixture=library");
    expect(fixture).not.toBeNull();

    const page = fixtureAssetPage(fixture as NonNullable<typeof fixture>, {
      sort: "file_name",
      direction: "asc",
      page: 1,
      pageSize: 200,
      filter: { ...emptyAssetFilter, ratings: [3] },
    });

    expect(page.total).toBe(1);
    expect(page.items[0]?.rating).toBe(4);
  });
});
