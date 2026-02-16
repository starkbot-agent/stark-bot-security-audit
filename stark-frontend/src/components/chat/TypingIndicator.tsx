export default function TypingIndicator() {
  return (
    <div className="flex justify-start mb-4">
      <div className="bg-slate-800 px-4 py-3 rounded-2xl rounded-bl-md">
        <div className="flex items-center gap-1.5">
          <div className="w-2 h-2 bg-slate-400 rounded-full typing-dot" />
          <div className="w-2 h-2 bg-slate-400 rounded-full typing-dot" />
          <div className="w-2 h-2 bg-slate-400 rounded-full typing-dot" />
        </div>
      </div>
    </div>
  );
}
