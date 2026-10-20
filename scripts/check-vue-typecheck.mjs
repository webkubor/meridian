// Run after installing dependencies: node scripts/check-vue-typecheck.mjs
// Verify both script and template diagnostics, not just compiler exit success.
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const require = createRequire(import.meta.url);
const cli = join(dirname(require.resolve('vue-tsc/package.json')), 'bin/vue-tsc.js');
const fixture = mkdtempSync(join(root, '.vue-typecheck-'));
try {
  writeFileSync(join(fixture, 'tsconfig.json'), JSON.stringify({
    extends: '../tsconfig.json',
    include: ['./Probe.vue'],
  }));
  for (const [name, source, diagnostic] of [
    ['valid SFC', '<script setup lang="ts">const count: number = 1;</script><template>{{ count.toFixed(2) }}</template>', null],
    ['script type error', '<script setup lang="ts">const count: string = 1;</script><template>{{ count }}</template>', 'TS2322'],
    ['template type error', '<script setup lang="ts">const count: number = 1;</script><template>{{ count.missingMethod() }}</template>', 'TS2339'],
  ]) {
    writeFileSync(join(fixture, 'Probe.vue'), source);
    const result = spawnSync(process.execPath, [cli, '--noEmit', '--pretty', 'false', '-p', join(fixture, 'tsconfig.json')], { cwd: root, encoding: 'utf8' });
    const output = `${result.stdout ?? ''}${result.stderr ?? ''}`;
    assert.ifError(result.error);
    if (diagnostic === null) {
      assert.equal(result.status, 0, output);
    } else {
      assert.equal(result.status, 2, output);
      assert.ok(output.includes('Probe.vue') && output.includes(diagnostic), output);
    }
    console.log(`PASS: ${name}`);
  }
} finally {
  rmSync(fixture, { recursive: true, force: true });
}
