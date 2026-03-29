import { useState } from "react";
import { Copy, Check } from "lucide-react";

interface CopyButtonProps {
  text: string;
  className?: string;
}

export function CopyButton({ text, className = "" }: CopyButtonProps) {
  const [copied, setCopied] = useState(false);

  async function handleCopy() {
    try {
      if (navigator.clipboard && window.isSecureContext) {
        await navigator.clipboard.writeText(text);
      } else {
        // fallback
        const textarea = document.createElement("textarea");
        textarea.value = text;
        textarea.style.position = "fixed";
        textarea.style.opacity = "0";
        document.body.appendChild(textarea);
        textarea.focus();
        textarea.select();
        document.execCommand("copy");
        document.body.removeChild(textarea);
      }

      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch (err) {
      console.error("Copy failed", err);
    }
  }

  return (
    <button
      onClick={handleCopy}
      className={`relative inline-flex items-center justify-center ${className}`}
      aria-label="Copy to clipboard"
    >
      {copied ? <Check size={14} /> : <Copy size={14} />}

      {copied && (
        <span className="absolute -top-6 text-xs bg-black text-white px-2 py-1 rounded shadow">
          Copied!
        </span>
      )}
    </button>
  );
}