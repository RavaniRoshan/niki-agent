import React, { useState } from 'react';
import { validateEmail } from '../../../shared';

export function NewsletterForm() {
  const [email, setEmail] = useState('');
  const [emailError, setEmailError] = useState<string | null>(null);
  const [subscribedEmail, setSubscribedEmail] = useState<string | null>(null);

  const handleSubscribe = (e: React.FormEvent) => {
    e.preventDefault();
    const result = validateEmail(email);
    if (!result.valid) {
      setEmailError(result.error || 'Please enter a valid email address.');
      return;
    }
    setEmailError(null);
    setSubscribedEmail(email.trim());
    setEmail('');
  };

  return (
    <form onSubmit={handleSubscribe} noValidate aria-label="Release updates signup" className="mt-5">
      <label htmlFor="footer-newsletter-email" className="sr-only">
        Your email
      </label>
      <input
        id="footer-newsletter-email"
        type="email"
        value={email}
        onChange={(e) => {
          setEmail(e.target.value);
          if (emailError) setEmailError(null);
        }}
        placeholder="Your email"
        className="w-full rounded-[5px] border border-input bg-card px-3.5 py-2.5 font-tech text-[12.5px] text-foreground placeholder:text-muted-foreground focus:border-ring focus:outline-none transition-colors"
      />

      {emailError && (
        <p role="alert" className="mt-2 font-tech text-[11px] text-destructive">
          {emailError}
        </p>
      )}

      {subscribedEmail && !emailError && (
        <p role="status" className="mt-2 font-tech text-[11px] text-chart-3 dark:text-chart-1">
          Subscribed {subscribedEmail} to Niki Agent releases.
        </p>
      )}

      <div className="mt-3.5 flex justify-end">
        <button
          type="submit"
          className="px-6 py-2.5 rounded-[5px] bg-primary text-primary-foreground hover:opacity-90 transition-opacity font-tech text-[12.5px] font-medium cursor-pointer"
        >
          Subscribe
        </button>
      </div>
    </form>
  );
}
