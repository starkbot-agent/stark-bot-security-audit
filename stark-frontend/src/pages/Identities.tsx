import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { Users, ChevronRight } from 'lucide-react';
import Card, { CardContent } from '@/components/ui/Card';
import { getIdentities } from '@/lib/api';

interface Identity {
  id: string;
  name: string;
  channel_type: string;
  platform_user_id: string;
  created_at: string;
}

export default function Identities() {
  const navigate = useNavigate();
  const [identities, setIdentities] = useState<Identity[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    loadIdentities();
  }, []);

  const loadIdentities = async () => {
    try {
      const data = await getIdentities();
      setIdentities(data);
    } catch (err) {
      setError('Failed to load identities');
    } finally {
      setIsLoading(false);
    }
  };

  const formatDate = (dateStr: string) => {
    return new Date(dateStr).toLocaleString();
  };

  if (isLoading) {
    return (
      <div className="p-8 flex items-center justify-center">
        <div className="flex items-center gap-3">
          <div className="w-6 h-6 border-2 border-stark-500 border-t-transparent rounded-full animate-spin" />
          <span className="text-slate-400">Loading identities...</span>
        </div>
      </div>
    );
  }

  return (
    <div className="p-8">
      <div className="mb-8">
        <h1 className="text-2xl font-bold text-white mb-2">Identities</h1>
        <p className="text-slate-400">View known user identities</p>
      </div>

      {error && (
        <div className="mb-6 bg-red-500/20 border border-red-500/50 text-red-400 px-4 py-3 rounded-lg">
          {error}
        </div>
      )}

      {identities.length > 0 ? (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {identities.map((identity) => (
            <Card
              key={identity.id}
              className="cursor-pointer hover:border-stark-500/50 transition-colors"
              onClick={() => navigate(`/identities/${identity.id}`)}
            >
              <CardContent>
                <div className="flex items-start gap-4">
                  <div className="p-3 bg-purple-500/20 rounded-lg shrink-0">
                    <Users className="w-6 h-6 text-purple-400" />
                  </div>
                  <div className="flex-1 min-w-0">
                    <h3 className="font-semibold text-white truncate">
                      {identity.name}
                    </h3>
                    <p className="text-sm text-slate-400 mt-1">
                      <span className="px-2 py-0.5 bg-slate-700 rounded text-xs mr-2">
                        {identity.channel_type}
                      </span>
                      {identity.platform_user_id}
                    </p>
                    <p className="text-xs text-slate-500 mt-2">
                      {formatDate(identity.created_at)}
                    </p>
                  </div>
                  <ChevronRight className="w-5 h-5 text-slate-500" />
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      ) : (
        <Card>
          <CardContent className="text-center py-12">
            <Users className="w-12 h-12 text-slate-600 mx-auto mb-4" />
            <p className="text-slate-400">No identities found</p>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
