using Realms;
using Realms.Logging;

RealmLogger.Default = RealmLogger.Null;
var root = args.Single();
Directory.CreateDirectory(root);
Create("full.realm", true);
Create("no-collections.realm", false);
Create("invalid-md5.realm", true, invalidHash: true);
Create("missing-name.realm", true, missingName: true);

void Create(string filename, bool collections, bool invalidHash = false, bool missingName = false)
{
    var configuration = new RealmConfiguration(Path.Combine(root, filename))
    {
        Schema = collections
            ? new[] { typeof(BeatmapSet), typeof(Beatmap), typeof(BeatmapCollection) }
            : new[] { typeof(BeatmapSet), typeof(Beatmap) },
    };
    using var realm = Realm.GetInstance(configuration);
    realm.Write(() =>
    {
        var set = new BeatmapSet { Hash = "Native-Set-Hash", OnlineID = 123 };
        set.Beatmaps.Add(new Beatmap
        {
            Hash = "Native-Hash-Is-NOT-MD5",
            MD5Hash = "ABCDEF0123456789ABCDEF0123456789",
            DifficultyName = "Hard",
        });
        set.Beatmaps.Add(new Beatmap
        {
            Hash = "Other-Native-Hash",
            MD5Hash = "0123456789ABCDEF0123456789ABCDEF",
            DifficultyName = "Easy",
        });
        realm.Add(set);
        if (!collections)
            return;
        var collection = new BeatmapCollection
        {
            ID = Guid.Parse("A1234567-89AB-CDEF-0123-456789ABCDEF"),
            Name = missingName ? null : "好きな曲 🎵",
        };
        collection.BeatmapMD5Hashes.Add(invalidHash ? "not-md5" : "ABCDEF0123456789ABCDEF0123456789");
        collection.BeatmapMD5Hashes.Add("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF");
        collection.BeatmapMD5Hashes.Add("ABCDEF0123456789ABCDEF0123456789");
        realm.Add(collection);
        realm.Add(new BeatmapCollection
        {
            ID = Guid.Parse("B1234567-89AB-CDEF-0123-456789ABCDEF"), Name = "",
        });
    });
}

public partial class BeatmapSet : IRealmObject
{
    public string? Hash { get; set; }
    public int OnlineID { get; set; }
    public IList<Beatmap> Beatmaps { get; } = null!;
}

public partial class Beatmap : IRealmObject
{
    public string? Hash { get; set; }
    public string? MD5Hash { get; set; }
    public string? DifficultyName { get; set; }
}

public partial class BeatmapCollection : IRealmObject
{
    [PrimaryKey]
    public Guid ID { get; set; }
    public string? Name { get; set; }
    public IList<string> BeatmapMD5Hashes { get; } = null!;
}
