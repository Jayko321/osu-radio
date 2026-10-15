using Realms;
using Realms.Logging;

RealmLogger.Default = RealmLogger.Null;
var root = args.Single();
Directory.CreateDirectory(root);
Create("full.realm", true);
Create("no-collections.realm", false);
Create("invalid-md5.realm", true, invalidHash: true);
Create("missing-name.realm", true, missingName: true);
CreateMetadata();
using (var realm = Realm.GetInstance(new RealmConfiguration(Path.Combine(root, "missing-beatmap-set.realm"))
{
    Schema = new[] { typeof(SchemaExtras) },
}))
{
    realm.Write(() => realm.Add(new SchemaExtras { ID = 1 }));
}

void CreateMetadata()
{
    var configuration = new RealmConfiguration(Path.Combine(root, "metadata.realm"))
    {
        Schema = new[] { typeof(RichBeatmapSet), typeof(RichBeatmap), typeof(Metadata),
            typeof(Author), typeof(NamedFileUsage), typeof(FileEntry), typeof(SchemaExtras) },
    };
    using var realm = Realm.GetInstance(configuration);
    realm.Write(() =>
    {
        var set = new RichBeatmapSet { OnlineID = (long)int.MaxValue + 1, Hash = "rich-set" };
        var metadata = new Metadata
        {
            Title = "Song", TitleUnicode = "曲 🎵", Artist = "Artist", ArtistUnicode = "演奏者",
            Source = "Game", Tags = "one two", PreviewTime = -123,
            AudioFile = "audio.mp3", BackgroundFile = "cover.jpg",
            Author = new Author { OnlineID = 456, Username = "Mapper", CountryCode = "EE" },
        };
        metadata.UserTags.Add("favorite");
        metadata.UserTags.Add("曲");
        set.Beatmaps.Add(new RichBeatmap { DifficultyName = "Hard", BPM = 180.5f, Metadata = metadata });
        set.Beatmaps.Add(new RichBeatmap
        {
            BPM = 120, Metadata = new Metadata { PreviewTime = long.MinValue,
                Author = new Author { OnlineID = long.MaxValue } },
        });
        set.Beatmaps.Add(new RichBeatmap());
        set.Beatmaps.Add(new RichBeatmap { Metadata = new Metadata() });
        foreach (var (filename, hash) in new (string?, string?)[]
        {
            ("audio.mp3", "abcdef1234"), ("cover.jpg", "12cover"),
            ("short", "x"), ("empty", ""), ("null hash", null),
        })
        {
            set.Files.Add(new NamedFileUsage { Filename = filename, File = new FileEntry { Hash = hash } });
        }
        set.Files.Add(new NamedFileUsage { Filename = "null file" });
        set.Files.Add(new NamedFileUsage { Filename = "two characters", File = new FileEntry { Hash = "12" } });
        realm.Add(set);
        realm.Add(new SchemaExtras { ID = 1, Label = "indexed" });
    });
}

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

[MapTo("BeatmapSet")]
public partial class RichBeatmapSet : IRealmObject
{
    public long OnlineID { get; set; }
    public string? Hash { get; set; }
    public IList<RichBeatmap> Beatmaps { get; } = null!;
    public IList<NamedFileUsage> Files { get; } = null!;
}

[MapTo("Beatmap")]
public partial class RichBeatmap : IRealmObject
{
    public string? DifficultyName { get; set; }
    public float BPM { get; set; }
    public Metadata? Metadata { get; set; }
}

public partial class Metadata : IRealmObject
{
    public string? Title { get; set; }
    public string? TitleUnicode { get; set; }
    public string? Artist { get; set; }
    public string? ArtistUnicode { get; set; }
    public Author? Author { get; set; }
    public string? Source { get; set; }
    public string? Tags { get; set; }
    public IList<string> UserTags { get; } = null!;
    public long PreviewTime { get; set; }
    public string? AudioFile { get; set; }
    public string? BackgroundFile { get; set; }
}

public partial class Author : IRealmObject
{
    public long OnlineID { get; set; }
    public string? Username { get; set; }
    public string? CountryCode { get; set; }
}

public partial class NamedFileUsage : IRealmObject
{
    public string? Filename { get; set; }
    public FileEntry? File { get; set; }
}

public partial class FileEntry : IRealmObject
{
    public string? Hash { get; set; }
}

public partial class SchemaExtras : IRealmObject
{
    [PrimaryKey]
    public int ID { get; set; }
    [Indexed]
    public string? Label { get; set; }
    public long? OptionalNumber { get; set; }
    public IList<string> Items { get; } = null!;
    public ISet<string> Tags { get; } = null!;
    public IDictionary<string, long?> Metrics { get; } = null!;
}
