export function report(summary) {
  const variants = summary.variants;
  const rate = (rows) => (rows.length ? `${Math.round((100 * rows.filter((row) => row.pass).length) / rows.length)}% ${rows.filter((row) => row.pass).length}/${rows.length}` : '-');
  const width = Math.max(14, ...variants.map((variant) => variant.label.length + 2));
  const table = (title, keysOf) => {
    const keys = [...new Set(variants.flatMap((variant) => variant.cases.flatMap(keysOf)))].sort();
    console.log(`\n${title.padEnd(26)}${variants.map((variant) => variant.label.padEnd(width)).join('')}`);
    for (const key of keys) {
      const cells = variants.map((variant) => rate(variant.cases.filter((row) => keysOf(row).includes(key))).padEnd(width));
      console.log(`${key.padEnd(26)}${cells.join('')}`);
    }
  };

  console.log(`\n${'model'.padEnd(26)}${'pass'.padEnd(14)}${'cost'.padEnd(10)}${'tokens in'.padEnd(14)}${'out'.padEnd(10)}per case`);
  for (const variant of variants) {
    const cost = variant.cases.reduce((sum, row) => sum + row.cost, 0);
    const input = variant.cases.reduce((sum, row) => sum + row.tokens.input, 0);
    const output = variant.cases.reduce((sum, row) => sum + row.tokens.output, 0);
    const mark = variant.cases.some((row) => row.costEstimated) ? '~' : '';
    const each = variant.cases.length ? `${mark}$${(cost / variant.cases.length).toFixed(3)}, ${Math.round(input / variant.cases.length / 1000)}k in` : '-';
    console.log(`${variant.label.padEnd(26)}${rate(variant.cases).padEnd(14)}${`${mark}$${cost.toFixed(2)}`.padEnd(10)}${input.toLocaleString().padEnd(14)}${output.toLocaleString().padEnd(10)}${each}`);
  }
  if (variants.some((variant) => variant.cases.some((row) => row.costEstimated))) {
    console.log('~ estimated from tokens at the provider price in lib/omp.mjs; the judge is counted at its real cost');
  }
  table('category', (row) => [row.category]);
  table('tool', (row) => row.tools);
  for (const variant of variants) {
    const failing = variant.cases.filter((row) => !row.pass);
    if (!failing.length) continue;
    console.log(`\nfailing on ${variant.label}:`);
    for (const row of failing) console.log(`  ${row.id}: ${row.failures.map((failure) => failure.why).join('; ')}`);
  }
}
