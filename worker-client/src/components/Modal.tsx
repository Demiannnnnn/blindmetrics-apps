import React, { useEffect, useId } from 'react';

interface ModalProps {
  title: string;
  isOpen: boolean;
  onClose: () => void;
  children: React.ReactNode;
  /** `md` for receipts and mark details. */
  size?: 'sm' | 'md';
  /** Esc closes the dialog (default true). Disabled while an action is in flight. */
  closeOnEscape?: boolean;
}

export const Modal: React.FC<ModalProps> = ({
  title,
  isOpen,
  onClose,
  children,
  size = 'sm',
  closeOnEscape = true,
}) => {
  const titleId = useId();

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && isOpen && closeOnEscape) {
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, onClose, closeOnEscape]);

  if (!isOpen) return null;

  return (
    <div
      className="fixed inset-0 bg-black/70 flex items-center justify-center p-4 z-50"
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
    >
      <div
        className={`bg-surface border border-default rounded-card w-full ${
          size === 'md' ? 'max-w-md' : 'max-w-sm'
        } max-h-[calc(100vh-2rem)] overflow-y-auto p-5 shadow-md text-fg`}
      >
        <h2 id={titleId} className="text-base font-semibold mb-3">
          {title}
        </h2>
        {children}
      </div>
    </div>
  );
};
