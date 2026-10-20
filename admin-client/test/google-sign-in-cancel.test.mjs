// admin-client/test/google-sign-in-cancel.test.mjs — Google sign-in cancellation tests (TASK-ADMIN-19, RF-ADMIN-19-3)

import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  shouldShowGoogleError,
  getGoogleErrorMessage,
  ERROR_MESSAGES,
} from '../src/lib/errors.ts';

test('google-sign-in-cancel: shouldShowGoogleError returns false when cancelled by user via button', () => {
  const errCancelled = { code: 'GOOGLE_SIGN_IN_CANCELLED' };
  assert.equal(shouldShowGoogleError(errCancelled, true), false);

  const errTimeout = { code: 'GOOGLE_SIGN_IN_TIMEOUT' };
  assert.equal(shouldShowGoogleError(errTimeout, true), false);

  const errUnknown = new Error('Random error');
  assert.equal(shouldShowGoogleError(errUnknown, true), false);
});

test('google-sign-in-cancel: shouldShowGoogleError returns true when not cancelled by in-app button', () => {
  const errCancelled = { code: 'GOOGLE_SIGN_IN_CANCELLED' };
  assert.equal(shouldShowGoogleError(errCancelled, false), true);

  const errTimeout = { code: 'GOOGLE_SIGN_IN_TIMEOUT' };
  assert.equal(shouldShowGoogleError(errTimeout, false), true);
});

test('google-sign-in-cancel: getGoogleErrorMessage returns null when cancelled by button', () => {
  const errCancelled = { code: 'GOOGLE_SIGN_IN_CANCELLED' };
  assert.equal(getGoogleErrorMessage(errCancelled, true), null);

  const errDenied = { reason: 'GOOGLE_SIGN_IN_CANCELLED' };
  assert.equal(getGoogleErrorMessage(errDenied, true), null);
});

test('google-sign-in-cancel: getGoogleErrorMessage returns Spanish text when Google yields access_denied', () => {
  // When Google itself returns error=access_denied in the browser redirect,
  // the cancellation was NOT from the in-app Cancel button.
  const errAccessDenied = { code: 'GOOGLE_SIGN_IN_CANCELLED' };
  const message = getGoogleErrorMessage(errAccessDenied, false);
  assert.equal(message, 'Cancelaste el inicio de sesión con Google.');
  assert.equal(message, ERROR_MESSAGES.GOOGLE_SIGN_IN_CANCELLED);
});

test('google-sign-in-cancel: getGoogleErrorMessage returns timeout error message', () => {
  const errTimeout = { code: 'GOOGLE_SIGN_IN_TIMEOUT' };
  const message = getGoogleErrorMessage(errTimeout, false);
  assert.equal(message, 'El navegador no volvió a tiempo. Inténtalo de nuevo.');
  assert.equal(message, ERROR_MESSAGES.GOOGLE_SIGN_IN_TIMEOUT);
});

test('google-sign-in-cancel: simulated UI button cancel flow leaves no error message', async () => {
  let waitingGoogle = false;
  let loading = false;
  let errorMessage = null;
  let cancelledByButton = false;

  // Step 1: User clicks "Continuar con Google"
  loading = true;
  waitingGoogle = true;
  errorMessage = null;
  cancelledByButton = false;

  // Step 2: User clicks "Cancelar" while waiting
  const onCancelClick = () => {
    cancelledByButton = true;
    loading = false;
    waitingGoogle = false;
    errorMessage = null;
  };

  onCancelClick();

  // Step 3: Rust login aborts and rejects with GOOGLE_SIGN_IN_CANCELLED
  const rustRejection = { code: 'GOOGLE_SIGN_IN_CANCELLED' };
  const errorToDisplay = getGoogleErrorMessage(rustRejection, cancelledByButton);
  if (errorToDisplay) {
    errorMessage = errorToDisplay;
  }

  assert.equal(waitingGoogle, false);
  assert.equal(loading, false);
  assert.equal(errorMessage, null, 'Error message must not be shown when cancelled by the button (RF-ADMIN-19-3)');
});

test('google-sign-in-cancel: simulated Google access_denied redirect displays error', async () => {
  let waitingGoogle = false;
  let loading = false;
  let errorMessage = null;
  let cancelledByButton = false;

  // Step 1: User clicks "Continuar con Google"
  loading = true;
  waitingGoogle = true;
  errorMessage = null;
  cancelledByButton = false;

  // Step 2: User denies authorization in Google browser page
  const rustRejection = { code: 'GOOGLE_SIGN_IN_CANCELLED' };
  const errorToDisplay = getGoogleErrorMessage(rustRejection, cancelledByButton);
  if (errorToDisplay) {
    errorMessage = errorToDisplay;
  }
  loading = false;
  waitingGoogle = false;

  assert.equal(waitingGoogle, false);
  assert.equal(loading, false);
  assert.equal(
    errorMessage,
    'Cancelaste el inicio de sesión con Google.',
    'Error message must be shown when Google returns access_denied without in-app button cancel'
  );
});
