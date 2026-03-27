import { useParams, useNavigate } from "react-router-dom";

export function SwapPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();

  return (
    <section className="mx-auto max-w-3xl p-6">
      <h1 className="mb-4 text-2xl font-bold">Swap Listing</h1>
      <p className="mb-6 text-slate-600">Preparing swap for listing #{id}. You can wire this into the real swap flow.</p>
      <button
        className="rounded-lg bg-slate-800 px-4 py-2 text-white hover:bg-slate-900"
        onClick={() => navigate(-1)}
      >
        Back
      </button>
    </section>
  );
}
