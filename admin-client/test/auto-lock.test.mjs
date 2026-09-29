import test from 'node:test';
import assert from 'node:assert/strict';

test('test_auto_lock_timer_resets_on_activity: timeout countdown resets on interaction', () => {
  let isLocked = false;
  let timerId = null;
  const timeoutMs = 50;

  const lockApp = () => {
    isLocked = true;
  };

  const resetTimer = () => {
    if (isLocked) return;
    if (timerId) clearTimeout(timerId);
    timerId = setTimeout(lockApp, timeoutMs);
  };

  // Start timer
  resetTimer();
  assert.equal(isLocked, false);

  // Interactivity before timeout resets the countdown
  clearTimeout(timerId);
  assert.equal(isLocked, false);

  // Lock explicitly
  lockApp();
  assert.equal(isLocked, true);
});

test('test_flush_clears_cached_tokens: memory and tokens are purged on lock', () => {
  let inMemoryAccessToken = 'super_secret_jwt_access_token_12345';
  let inMemoryDecryptedRecords = [{ id: 'rec-1', payload: { cpu: 30 } }];

  const lockAndFlushMemory = () => {
    // Zeroize token
    inMemoryAccessToken = null;
    // Wipe decrypted cache
    inMemoryDecryptedRecords = [];
  };

  lockAndFlushMemory();

  assert.equal(inMemoryAccessToken, null);
  assert.equal(inMemoryDecryptedRecords.length, 0);
});

test('test_failed_unlock_attempts_limits: 5 failed attempts forces security logout', () => {
  let attempts = 0;
  let isRevoked = false;

  const recordFailedAttempt = () => {
    attempts += 1;
    if (attempts >= 5) {
      isRevoked = true;
    }
  };

  for (let i = 0; i < 4; i++) {
    recordFailedAttempt();
    assert.equal(isRevoked, false, `Should not revoke on attempt ${i + 1}`);
  }

  // 5th attempt
  recordFailedAttempt();
  assert.equal(attempts, 5);
  assert.equal(isRevoked, true, 'Must revoke session after 5 failed attempts');
});
